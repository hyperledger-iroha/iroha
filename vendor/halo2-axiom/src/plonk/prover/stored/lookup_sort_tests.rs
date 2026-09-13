//! Independent stored sorting oracles, protocol-owner preservation and terminal fault cleanup.
//!
//! The in-memory fixture keeps plaintext oracle copies. These tests cover only sorted usable
//! prefixes, not lookup membership/permutation, cryptographic storage, complete proofs or RSS.

use super::super::super::super::lookup_sort::{
    SortedLookupV1, scratch_bytes, take_clear_observations,
};
use super::*;
use crate::plonk::lookup::prover::stored_sort_ordinary_oracle;

fn sorting_oracle<C>()
where
    C: CurveAffine + crate::SerdeCurveAffine,
    C::Scalar: StoredAssignmentFieldV1
        + WithSmallOrderMulGroup<3>
        + FromUniformBytes<64>
        + crate::SerdePrimeField,
{
    for (k, selectors) in [(4, true), (8, false), (9, true), (10, false)] {
        let params = ParamsIPA::<C>::new(k);
        let pk = lookup_key(&params, selectors);
        let usable = (1_usize << k) - (pk.vk.cs.blinding_factors() + 1);
        let shared = Shared::<C>::new();
        let values = columns::<C::Scalar>();
        let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let mut compressed =
            prepare_single_phase_stored_ipa_prefix_v1::<C, _, _, _, Challenge255<C>, _, true, 6>(
                &params,
                pk,
                LookupProducer(Producer::new(&shared, if k < 9 { 3 } else { 300 })),
                &instances,
                Provider::new(&shared),
                Rng(Arc::clone(&shared)),
                RecordingTranscript::new(&shared),
            )
            .unwrap()
            .stage_advice_coefficients()
            .unwrap()
            .compress_lookups(1 << 20)
            .unwrap();
        let mut expected = Vec::new();
        let mut originals = Vec::new();
        for pair in &mut compressed.lookups {
            for column in [&mut pair.input, &mut pair.table] {
                let values = read_compressed::<C, _>(&mut column.snapshot, column.layout);
                let ordinary =
                    stored_sort_ordinary_oracle(&compressed.inner.pk, &params, values.clone());
                expected.push(ordinary[..usable].to_vec());
                originals.push((column.layout, values));
            }
        }
        let theta = *compressed.theta;
        let lookup_allocation = compressed.lookups.as_ptr();
        let fixed_allocation = compressed.inner.pk.fixed_values.as_ptr();
        let vk = compressed
            .inner
            .pk
            .get_vk()
            .to_bytes(crate::SerdeFormat::Processed);
        let advice_layouts = compressed
            .inner
            .advice
            .layouts()
            .unwrap()
            .collect::<Vec<_>>();
        let challenges = compressed
            .inner
            .advice
            .challenges()
            .unwrap()
            .collect::<Vec<_>>();
        let (events, draws, sealed) = {
            let log = shared.log.lock().unwrap();
            (log.events.clone(), log.rng_calls, log.sealed.clone())
        };
        let mut expected_rng = shared.rng.lock().unwrap().clone();
        take_clear_observations();
        let budget = scratch_bytes::<C::Scalar, Snapshot<C>>(2).unwrap();
        assert_eq!(std::mem::size_of::<C::Scalar>(), 32);
        assert_eq!(
            budget,
            32_768 + 2 * std::mem::size_of::<SortedLookupV1<Snapshot<C>>>()
        );
        let mut sorted = compressed.sort_lookup_values(budget).unwrap();
        assert_eq!(sorted.usable_rows, usable);
        assert_eq!(sorted.sorted.len(), 2);
        assert_eq!(sorted.compressed.lookups.as_ptr(), lookup_allocation);
        assert_eq!(*sorted.compressed.theta, theta);
        assert!(std::ptr::eq(sorted.compressed.inner.params, &params));
        assert_eq!(
            sorted.compressed.inner.instances.as_ptr(),
            instances.as_ptr()
        );
        assert_eq!(
            sorted.compressed.inner.pk.fixed_values.as_ptr(),
            fixed_allocation
        );
        assert_eq!(
            sorted
                .compressed
                .inner
                .pk
                .get_vk()
                .to_bytes(crate::SerdeFormat::Processed),
            vk
        );
        assert_eq!(
            sorted
                .compressed
                .inner
                .advice
                .layouts()
                .unwrap()
                .collect::<Vec<_>>(),
            advice_layouts
        );
        assert_eq!(
            sorted
                .compressed
                .inner
                .advice
                .challenges()
                .unwrap()
                .collect::<Vec<_>>(),
            challenges
        );
        assert!(Arc::ptr_eq(
            &sorted.compressed.inner.provider.shared,
            &shared
        ));
        assert!(Arc::ptr_eq(&sorted.compressed.inner.rng.0, &shared));
        assert!(Arc::ptr_eq(
            &sorted.compressed.inner.transcript.shared,
            &shared
        ));
        for (lookup, pair) in sorted.sorted.iter_mut().enumerate() {
            for (side_index, (side, column)) in [
                (StoredLookupSideV1::Input, &mut pair.input),
                (StoredLookupSideV1::Table, &mut pair.table),
            ]
            .into_iter()
            .enumerate()
            {
                assert_eq!(
                    column.layout.role(),
                    StoredPolynomialRoleV1::LookupSorted {
                        lookup: lookup as u32,
                        side,
                        run_log: k
                    }
                );
                assert_eq!(column.layout.basis(), StoredPolynomialBasisV1::Lagrange);
                assert_eq!(column.layout.k(), k);
                assert_eq!(column.layout.field(), C::Scalar::STORED_FIELD);
                assert!(column.layout.same_proof_context(originals[0].0));
                assert!(column.layout.ordinal() > originals.last().unwrap().0.ordinal());
                let actual = read_compressed::<C, _>(&mut column.snapshot, column.layout);
                assert_eq!(&actual[..usable], &expected[2 * lookup + side_index]);
                assert!(
                    actual[usable..]
                        .iter()
                        .all(|value| *value == C::Scalar::ZERO)
                );
            }
        }
        for (lookup, pair) in sorted.compressed.lookups.iter_mut().enumerate() {
            for (side, column) in [&mut pair.input, &mut pair.table].into_iter().enumerate() {
                assert_eq!(column.layout, originals[2 * lookup + side].0);
                assert_eq!(
                    read_compressed::<C, _>(&mut column.snapshot, column.layout),
                    originals[2 * lookup + side].1
                );
            }
        }
        {
            let log = shared.log.lock().unwrap();
            assert_eq!(log.events, events);
            assert_eq!(log.rng_calls, draws);
            assert_eq!(log.sealed[..sealed.len()], sealed);
            let passes = usize::try_from(k - k.min(8) + 1).unwrap();
            assert_eq!(log.created, 8 + 4 * passes);
            assert_eq!(log.sealed.len(), log.created);
            assert_eq!(log.writer_drops, log.created);
            assert_eq!(log.snapshot_drops, 4 * (passes - 1));
            assert_eq!(
                (log.provider_drops, log.rng_drops, log.transcript_drops),
                (0, 0, 0)
            );
            for (layout, encoded) in &log.sealed[sealed.len()..] {
                let StoredPolynomialRoleV1::LookupSorted {
                    lookup,
                    side,
                    run_log,
                } = layout.role()
                else {
                    panic!("non-sorted intermediate role")
                };
                assert!((k.min(8)..=k).contains(&run_log));
                let original = &originals
                    [2 * lookup as usize + usize::from(side == StoredLookupSideV1::Table)]
                .1;
                for start in (0..usable).step_by(1_usize << run_log) {
                    let end = (start + (1_usize << run_log)).min(usable);
                    let mut run = original[start..end].to_vec();
                    run.sort();
                    let actual = encoded[start..end]
                        .iter()
                        .map(|value| {
                            Option::<C::Scalar>::from(C::Scalar::from_repr(*value)).unwrap()
                        })
                        .collect::<Vec<_>>();
                    assert_eq!(actual, run);
                }
                assert!(encoded[usable..].iter().all(|value| *value == [0; 32]));
            }
        }
        let (field_slots, field_zero, byte_slots, byte_zero) = take_clear_observations();
        assert!(field_slots >= 3 * 256 && byte_slots >= 256);
        assert!(field_zero && byte_zero);
        let (mut next, mut expected_next) = ([0; 64], [0; 64]);
        sorted.compressed.inner.rng.fill_bytes(&mut next);
        expected_rng.fill_bytes(&mut expected_next);
        assert_eq!(next, expected_next);
        drop(sorted);
        assert_dropped(&shared);
        let log = shared.log.lock().unwrap();
        assert_eq!(log.snapshot_drops, log.sealed.len());
    }
}

#[test]
fn eq_sorted_values_match_active_ordinary_helper_and_retain_original_protocol_owners() {
    sorting_oracle::<EqAffine>();
}

#[test]
fn ep_sorted_values_match_active_ordinary_helper_and_retain_original_protocol_owners() {
    sorting_oracle::<EpAffine>();
}

struct PatternLookupProducer<C: CurveAffine, const NONZERO: bool>(EmptyLookupProducer<C>);
impl<C: CurveAffine, const NONZERO: bool> Circuit<C::Scalar> for PatternLookupProducer<C, NONZERO> {
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
        meta.lookup_any("sorting-only duplicate and missing value fixture", |meta| {
            let value = meta.query_instance(instance, Rotation::cur());
            let base = C::Scalar::from(if NONZERO { 5 } else { 0 });
            vec![(
                value.clone() + Expression::Constant(base),
                value + Expression::Constant(base + C::Scalar::ONE),
            )]
        });
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

fn pattern_sort<C, const NONZERO: bool>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    for k in [4, 8, 9, 10] {
        let params = ParamsIPA::<C>::new(k);
        let key_shared = Shared::<C>::new();
        let producer =
            PatternLookupProducer::<C, NONZERO>(EmptyLookupProducer(Producer::new(&key_shared, 0)));
        let vk = keygen_vk_custom(&params, &producer, true).unwrap();
        let pk = keygen_pk(&params, vk, &producer).unwrap();
        let usable = (1 << k) - (pk.vk.cs.blinding_factors() + 1);
        let mut values = vec![C::Scalar::ZERO; usable];
        for (i, value) in values.iter_mut().enumerate() {
            if i % 7 == 0 {
                *value = C::Scalar::from(13);
            }
        }
        values[0] = C::Scalar::ZERO;
        // Distinguish field ordering from lexicographic little-endian representation bytes.
        values[1] = C::Scalar::from(256);
        values[2] = C::Scalar::from(257);
        values[3] = C::Scalar::from(65_536);
        values[4] = -C::Scalar::from(100);
        values[usable - 1] = C::Scalar::from(21);
        let instances: [&[C::Scalar]; 1] = [&values];
        let shared = Shared::<C>::new();
        let compressed =
            prepare_single_phase_stored_ipa_prefix_v1::<C, _, _, _, Challenge255<C>, _, false, 0>(
                &params,
                pk,
                PatternLookupProducer::<C, NONZERO>(EmptyLookupProducer(Producer::new(&shared, 0))),
                &instances,
                Provider::new(&shared),
                Rng(Arc::clone(&shared)),
                RecordingTranscript::new(&shared),
            )
            .unwrap()
            .stage_advice_coefficients()
            .unwrap()
            .compress_lookups(1 << 20)
            .unwrap();
        let (events, draws) = {
            let log = shared.log.lock().unwrap();
            (log.events.clone(), log.rng_calls)
        };
        let input = compressed.lookups[0]
            .input
            .snapshot
            .values
            .iter()
            .map(|value| Option::<C::Scalar>::from(C::Scalar::from_repr(*value)).unwrap())
            .collect::<Vec<_>>();
        let table = compressed.lookups[0]
            .table
            .snapshot
            .values
            .iter()
            .map(|value| Option::<C::Scalar>::from(C::Scalar::from_repr(*value)).unwrap())
            .collect::<Vec<_>>();
        let missing = C::Scalar::from(if NONZERO { 5 } else { 0 });
        assert!(input[..usable].contains(&missing));
        assert!(!table[..usable].contains(&missing));
        assert!(input[..usable].windows(2).any(|pair| pair[0] == pair[1]));
        assert_eq!(input[..usable].contains(&C::Scalar::ZERO), !NONZERO);
        let expected_input = stored_sort_ordinary_oracle(&compressed.inner.pk, &params, input);
        let expected_table = stored_sort_ordinary_oracle(&compressed.inner.pk, &params, table);
        let mut sorted = compressed
            .sort_lookup_values(scratch_bytes::<C::Scalar, Snapshot<C>>(1).unwrap())
            .unwrap();
        assert_eq!(sorted.usable_rows, usable);
        for (column, expected) in [(&mut sorted.sorted[0].input, &expected_input)] {
            let actual = read_compressed::<C, _>(&mut column.snapshot, column.layout);
            assert_eq!(&actual[..usable], &expected[..usable]);
            assert!(
                actual[usable..]
                    .iter()
                    .all(|value| *value == C::Scalar::ZERO)
            );
            if NONZERO {
                assert!(
                    actual[..usable]
                        .iter()
                        .all(|value| *value != C::Scalar::ZERO)
                );
            }
        }
        let column = &mut sorted.sorted[0].table;
        let actual = read_compressed::<C, _>(&mut column.snapshot, column.layout);
        assert_eq!(&actual[..usable], &expected_table[..usable]);
        assert!(
            actual[usable..]
                .iter()
                .all(|value| *value == C::Scalar::ZERO)
        );
        // Missing membership is deliberately retained for the later permutation stage.
        assert!(expected_input[..usable].contains(&missing));
        assert!(!actual[..usable].contains(&missing));
        {
            let log = shared.log.lock().unwrap();
            assert_eq!(log.events, events);
            assert_eq!(log.rng_calls, draws);
        }
        drop(sorted);
        assert_dropped(&shared);
    }
}

#[test]
fn both_pasta_duplicates_zeros_missing_values_and_partial_runs_keep_exact_usable_population() {
    pattern_sort::<EqAffine, false>();
    pattern_sort::<EpAffine, false>();
    pattern_sort::<EqAffine, true>();
    pattern_sort::<EpAffine, true>();
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SortChange {
    Context,
    Ordinal,
    Lookup,
    Side,
    Pass,
    CompressedRole,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SortFault {
    None,
    Create(u64),
    Write(u64, u64),
    Seal(u64),
    Read(u64, u64),
    PanicWrite(u64, u64),
    PanicSeal(u64),
    PanicRead(u64, u64),
    Encoding(u64, u64),
    ShortChunk(u64, u64),
    CreateLayout(SortChange),
    WriterSecondObservation,
    WriterAfterWrite,
    SnapshotLayout(u64, SortChange),
    AfterSeal(u64, u64),
    AfterRead(u64, u64, u64),
}
fn changed_sort_layout(
    layout: StoredPolynomialLayoutV1,
    change: SortChange,
) -> StoredPolynomialLayoutV1 {
    let StoredPolynomialRoleV1::LookupSorted {
        lookup,
        side,
        run_log,
    } = layout.role()
    else {
        return different_context(layout);
    };
    let role = match change {
        SortChange::Lookup => StoredPolynomialRoleV1::LookupSorted {
            lookup: lookup + 1,
            side,
            run_log,
        },
        SortChange::Side => StoredPolynomialRoleV1::LookupSorted {
            lookup,
            side: if side == StoredLookupSideV1::Input {
                StoredLookupSideV1::Table
            } else {
                StoredLookupSideV1::Input
            },
            run_log,
        },
        SortChange::Pass => StoredPolynomialRoleV1::LookupSorted {
            lookup,
            side,
            run_log: if run_log == layout.k() {
                run_log - 1
            } else {
                run_log + 1
            },
        },
        SortChange::CompressedRole => StoredPolynomialRoleV1::LookupCompressed { lookup, side },
        _ => layout.role(),
    };
    StoredPolynomialLayoutV1::new(
        if change == SortChange::Context {
            [41; 32]
        } else {
            [23; 32]
        },
        if change == SortChange::Ordinal {
            layout.ordinal() - 1
        } else {
            layout.ordinal()
        },
        layout.field(),
        layout.basis(),
        layout.k(),
        role,
    )
    .unwrap()
}
struct SortProvider<C: CurveAffine> {
    inner: Provider<C>,
    fault: Arc<Mutex<SortFault>>,
    window: Arc<std::sync::atomic::AtomicBool>,
}
struct SortWriter<C: CurveAffine> {
    inner: Writer<C>,
    fault: Arc<Mutex<SortFault>>,
    window: Arc<std::sync::atomic::AtomicBool>,
    observations: std::cell::Cell<usize>,
}
struct SortSnapshot<C: CurveAffine> {
    inner: Snapshot<C>,
    fault: Arc<Mutex<SortFault>>,
    window: Arc<std::sync::atomic::AtomicBool>,
}
impl<C: CurveAffine> StoredPolynomialProviderV1 for SortProvider<C> {
    type Writer = SortWriter<C>;
    fn create(
        &mut self,
        field: StoredPastaFieldV1,
        basis: StoredPolynomialBasisV1,
        k: u32,
        role: StoredPolynomialRoleV1,
    ) -> Result<Self::Writer, StoredPolynomialErrorV1> {
        assert!(!self.window.load(std::sync::atomic::Ordering::SeqCst));
        let fault = *self.fault.lock().unwrap();
        if fault == SortFault::Create(self.inner.ordinal) {
            return Err(StoredPolynomialErrorV1::Storage);
        }
        let mut inner = self.inner.create(field, basis, k, role)?;
        if inner.layout.ordinal() == 8 {
            if let SortFault::CreateLayout(change) = fault {
                inner.layout = changed_sort_layout(inner.layout, change);
            }
        }
        Ok(SortWriter {
            inner,
            fault: Arc::clone(&self.fault),
            window: Arc::clone(&self.window),
            observations: std::cell::Cell::new(0),
        })
    }
}
impl<C: CurveAffine> StoredPolynomialWriterV1 for SortWriter<C> {
    type Snapshot = SortSnapshot<C>;
    fn layout(&self) -> StoredPolynomialLayoutV1 {
        let observation = self.observations.get();
        self.observations.set(observation + 1);
        let fault = *self.fault.lock().unwrap();
        if self.inner.layout.ordinal() == 8
            && ((fault == SortFault::WriterSecondObservation && observation > 0)
                || (fault == SortFault::WriterAfterWrite && self.inner.next > 0))
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
        let fault = *self.fault.lock().unwrap();
        let ordinal = self.inner.layout.ordinal();
        assert_ne!(fault, SortFault::PanicWrite(ordinal, chunk));
        if fault == SortFault::Write(ordinal, chunk) {
            return Err(StoredPolynomialErrorV1::Storage);
        }
        self.inner.write_chunk(chunk, values)
    }
    fn seal(self) -> Result<Self::Snapshot, StoredPolynomialErrorV1> {
        assert!(!self.window.load(std::sync::atomic::Ordering::SeqCst));
        let fault = *self.fault.lock().unwrap();
        let ordinal = self.inner.layout.ordinal();
        assert_ne!(fault, SortFault::PanicSeal(ordinal));
        if fault == SortFault::Seal(ordinal) {
            return Err(StoredPolynomialErrorV1::Storage);
        }
        let inner = self.inner.seal()?;
        if let SortFault::AfterSeal(trigger, victim) = fault {
            if ordinal == trigger {
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
        Ok(SortSnapshot {
            inner,
            fault: self.fault,
            window: self.window,
        })
    }
}
impl<C: CurveAffine> StoredPolynomialSnapshotV1 for SortSnapshot<C> {
    fn layout(&self) -> StoredPolynomialLayoutV1 {
        match *self.fault.lock().unwrap() {
            SortFault::SnapshotLayout(ordinal, change)
                if ordinal == self.inner.layout.ordinal() =>
            {
                changed_sort_layout(self.inner.layout, change)
            }
            _ => self.inner.layout(),
        }
    }
    fn with_chunk<V>(
        &mut self,
        expected: StoredPolynomialLayoutV1,
        chunk: u64,
        consume: impl FnOnce(&[[u8; 32]]) -> Result<V, StoredPolynomialErrorV1>,
    ) -> Result<V, StoredPolynomialErrorV1> {
        let fault = *self.fault.lock().unwrap();
        let ordinal = self.inner.layout.ordinal();
        assert!(!self.window.swap(true, std::sync::atomic::Ordering::SeqCst));
        let _window = ReadWindow(Arc::clone(&self.window));
        if fault == SortFault::Read(ordinal, chunk) {
            return Err(StoredPolynomialErrorV1::Storage);
        }
        let result = self.inner.with_chunk(expected, chunk, |values| {
            assert_ne!(fault, SortFault::PanicRead(ordinal, chunk));
            if matches!(fault, SortFault::Encoding(o, c) | SortFault::ShortChunk(o, c) if o == ordinal && c == chunk) {
                let mut changed = values.to_vec();
                if fault == SortFault::Encoding(ordinal, chunk) { changed[0] = [0xff; 32]; } else { changed.pop(); }
                consume(&changed)
            } else { consume(values) }
        });
        if let SortFault::AfterRead(trigger, target_chunk, victim) = fault {
            if ordinal == trigger && chunk == target_chunk {
                let mut log = self.inner.shared.log.lock().unwrap();
                let original = log
                    .sealed
                    .iter()
                    .find(|(layout, _)| layout.ordinal() == victim)
                    .unwrap()
                    .0;
                log.snapshot_layout_override = Some((victim, different_context(original)));
            }
        }
        result
    }
    fn with_column<V>(
        &mut self,
        _: StoredPolynomialLayoutV1,
        _: impl FnOnce(&[[u8; 32]]) -> Result<V, StoredPolynomialErrorV1>,
    ) -> Result<V, StoredPolynomialErrorV1> {
        panic!("sorting requested an unbounded full column")
    }
}

fn sort_faults<C>(metadata: bool)
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(9);
    let pk = lookup_key(&params, true);
    let values = columns::<C::Scalar>();
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let faults = if metadata {
        vec![
            SortFault::CreateLayout(SortChange::Context),
            SortFault::CreateLayout(SortChange::Ordinal),
            SortFault::CreateLayout(SortChange::Lookup),
            SortFault::CreateLayout(SortChange::Side),
            SortFault::CreateLayout(SortChange::Pass),
            SortFault::CreateLayout(SortChange::CompressedRole),
            SortFault::WriterSecondObservation,
            SortFault::WriterAfterWrite,
            SortFault::SnapshotLayout(8, SortChange::Pass),
            SortFault::SnapshotLayout(9, SortChange::Side),
            SortFault::AfterSeal(15, 0),
            SortFault::AfterSeal(15, 2),
            SortFault::AfterSeal(15, 4),
            SortFault::AfterSeal(15, 9),
            SortFault::AfterRead(4, 0, 0),
            SortFault::AfterRead(4, 0, 4),
            SortFault::AfterRead(8, 0, 4),
            SortFault::AfterRead(8, 0, 8),
        ]
    } else {
        vec![
            SortFault::Create(12),
            SortFault::Write(8, 1),
            SortFault::Write(9, 1),
            SortFault::Seal(10),
            SortFault::Read(4, 1),
            SortFault::Read(8, 1),
            SortFault::PanicWrite(8, 1),
            SortFault::PanicWrite(9, 1),
            SortFault::PanicSeal(10),
            SortFault::PanicRead(4, 1),
            SortFault::PanicRead(8, 1),
            SortFault::Encoding(4, 1),
            SortFault::Encoding(8, 1),
            SortFault::ShortChunk(4, 1),
            SortFault::ShortChunk(8, 1),
        ]
    };
    for fault in faults {
        let shared = Shared::<C>::new();
        let control = Arc::new(Mutex::new(SortFault::None));
        let window = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let compressed =
            prepare_single_phase_stored_ipa_prefix_v1::<C, _, _, _, Challenge255<C>, _, true, 6>(
                &params,
                pk.clone(),
                LookupProducer(Producer::new(&shared, 300)),
                &instances,
                SortProvider {
                    inner: Provider::new(&shared),
                    fault: Arc::clone(&control),
                    window: Arc::clone(&window),
                },
                Rng(Arc::clone(&shared)),
                RecordingTranscript::new(&shared),
            )
            .unwrap()
            .stage_advice_coefficients()
            .unwrap()
            .compress_lookups(1 << 20)
            .unwrap();
        let (events, draws, reads, writes, prior) = {
            let log = shared.log.lock().unwrap();
            (
                log.events.clone(),
                log.rng_calls,
                log.reads,
                log.writes,
                log.sealed.clone(),
            )
        };
        take_clear_observations();
        *control.lock().unwrap() = fault;
        let result = catch_unwind(AssertUnwindSafe(|| {
            compressed
                .sort_lookup_values(scratch_bytes::<C::Scalar, SortSnapshot<C>>(2).unwrap())
                .map(|_| ())
        }));
        if matches!(
            fault,
            SortFault::PanicWrite(..) | SortFault::PanicSeal(..) | SortFault::PanicRead(..)
        ) {
            assert!(result.is_err(), "fault was not reached: {fault:?}");
        } else {
            let error = result
                .unwrap()
                .expect_err("sorting admitted substituted metadata or failed storage");
            if metadata {
                assert!(
                    matches!(
                        error,
                        StoredLookupErrorV1::Store(StoredPolynomialErrorV1::Context)
                            | StoredLookupErrorV1::Phase(StoredPhaseErrorV1::Store(
                                StoredPolynomialErrorV1::Context
                            ))
                            | StoredLookupErrorV1::Context
                    ),
                    "{fault:?}: {error:?}"
                );
            }
        }
        assert!(
            !window.load(std::sync::atomic::Ordering::SeqCst),
            "borrow window escaped {fault:?}"
        );
        assert_dropped(&shared);
        let log = shared.log.lock().unwrap();
        assert_eq!(log.events, events, "{fault:?} changed transcript");
        assert_eq!(log.rng_calls, draws, "{fault:?} drew proof randomness");
        assert_eq!(log.sealed[..prior.len()], prior);
        assert_eq!(log.writer_drops, log.created);
        assert_eq!(
            log.snapshot_drops,
            log.sealed.len(),
            "partial owner escaped {fault:?}"
        );
        if matches!(
            fault,
            SortFault::CreateLayout(_) | SortFault::WriterSecondObservation
        ) {
            assert_eq!(
                log.reads, reads,
                "unadmitted destination read source: {fault:?}"
            );
            assert_eq!(log.writes, writes);
            assert_eq!(log.sealed.len(), 8);
        }
        if matches!(fault, SortFault::AfterSeal(..)) {
            assert_eq!(log.sealed.len(), 16);
        }
        let (fields, field_zero, bytes, byte_zero) = take_clear_observations();
        assert!(
            fields >= 256 && bytes >= 256,
            "{fault:?} did not observe initialized scratch cleanup"
        );
        assert!(
            field_zero && byte_zero,
            "{fault:?} left initialized scratch values"
        );
    }
}

#[test]
fn both_pasta_sorted_pass_layout_and_post_seal_owner_substitutions_fail_closed() {
    sort_faults::<EqAffine>(true);
    sort_faults::<EpAffine>(true);
}
#[test]
fn both_pasta_partial_initial_and_merge_faults_unwind_drop_owners_and_clear_initialized_scratch() {
    sort_faults::<EqAffine>(false);
    sort_faults::<EpAffine>(false);
}

fn sort_preflights<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(4);
    let wrong_params = ParamsIPA::<C>::new(5);
    let pk = lookup_key(&params, true);
    let values = columns::<C::Scalar>();
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    for fault in 0..8 {
        let shared = Shared::<C>::new();
        let mut compressed =
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
            .unwrap()
            .compress_lookups(1 << 20)
            .unwrap();
        let mut budget = scratch_bytes::<C::Scalar, Snapshot<C>>(2).unwrap();
        match fault {
            0 => budget = 0,
            1 => budget -= 1,
            2 => {
                compressed.lookups.pop();
            }
            3 => {
                let pair = &mut compressed.lookups[0];
                std::mem::swap(&mut pair.input, &mut pair.table);
            }
            4 => compressed.inner.params = &wrong_params,
            5 => {
                // A valid monotonic receipt can still exhaust planned writer ordinals.
                let column = &mut compressed.lookups[1].table;
                let layout = StoredPolynomialLayoutV1::new(
                    [23; 32],
                    u64::MAX,
                    column.layout.field(),
                    column.layout.basis(),
                    column.layout.k(),
                    column.layout.role(),
                )
                .unwrap();
                column.layout = layout;
                column.snapshot.layout = layout;
            }
            6 | 7 => {
                let mut log = shared.log.lock().unwrap();
                let layout = log.sealed[if fault == 6 { 2 } else { 4 }].0;
                log.snapshot_layout_override = Some((layout.ordinal(), different_context(layout)));
            }
            _ => unreachable!(),
        }
        let (events, draws, reads, writes, created) = {
            let log = shared.log.lock().unwrap();
            (
                log.events.clone(),
                log.rng_calls,
                log.reads,
                log.writes,
                log.created,
            )
        };
        take_clear_observations();
        let result = compressed.sort_lookup_values(budget).map(|_| ());
        if fault <= 1 {
            assert_eq!(result, Err(StoredLookupErrorV1::ScratchLimit));
        } else {
            assert!(result.is_err(), "preflight {fault}");
        }
        assert_dropped(&shared);
        let log = shared.log.lock().unwrap();
        assert_eq!(log.events, events);
        assert_eq!(log.rng_calls, draws);
        assert_eq!(log.reads, reads, "preflight {fault} reached witness bytes");
        assert_eq!(log.writes, writes);
        assert_eq!(log.created, created);
        assert_eq!(log.snapshot_drops, 8);
        assert_eq!(take_clear_observations(), (0, true, 0, true));
    }
}
#[test]
fn both_pasta_sort_budget_and_original_key_receipts_preflight_before_reads_or_writes() {
    sort_preflights::<EqAffine>();
    sort_preflights::<EpAffine>();
}

fn empty_sort<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(4);
    let key_shared = Shared::<C>::new();
    let producer =
        SparseLookupProducer::<C, true>(EmptyLookupProducer(Producer::new(&key_shared, 0)));
    let vk = keygen_vk_custom(&params, &producer, true).unwrap();
    let pk = keygen_pk(&params, vk, &producer).unwrap();
    let shared = Shared::<C>::new();
    let values = vec![C::Scalar::from(17)];
    let instances: [&[C::Scalar]; 1] = [&values];
    let compressed =
        prepare_single_phase_stored_ipa_prefix_v1::<C, _, _, _, Challenge255<C>, _, false, 0>(
            &params,
            pk,
            SparseLookupProducer::<C, true>(EmptyLookupProducer(Producer::new(&shared, 0))),
            &instances,
            Provider::new(&shared),
            Rng(Arc::clone(&shared)),
            RecordingTranscript::new(&shared),
        )
        .unwrap()
        .stage_advice_coefficients()
        .unwrap()
        .compress_lookups(0)
        .unwrap();
    let events = shared.log.lock().unwrap().events.clone();
    take_clear_observations();
    assert_eq!(scratch_bytes::<C::Scalar, Snapshot<C>>(0).unwrap(), 0);
    assert_eq!(
        scratch_bytes::<C::Scalar, Snapshot<C>>(usize::MAX),
        Err(StoredLookupErrorV1::Context)
    );
    let sorted = compressed.sort_lookup_values(0).unwrap();
    assert!(sorted.sorted.is_empty());
    assert!(sorted.compressed.lookups.is_empty());
    assert_eq!(take_clear_observations(), (0, true, 0, true));
    {
        let log = shared.log.lock().unwrap();
        assert_eq!(log.events, events);
        assert_eq!(log.rng_calls, 0);
        assert_eq!(log.created, 0);
        assert_eq!(log.reads, 0);
        assert_eq!(log.writes, 0);
    }
    drop(sorted);
    assert_dropped(&shared);
}
#[test]
fn both_pasta_no_lookups_need_zero_sort_scratch_without_store_rng_or_transcript_activity() {
    empty_sort::<EqAffine>();
    empty_sort::<EpAffine>();
}
