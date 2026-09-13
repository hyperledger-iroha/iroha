//! Independent membership/leftover oracles and consuming-owner failure checks.
//!
//! Plaintext fixture banks are test oracles, not production storage. This suite exercises only
//! the membership continuation; no final permutation, proof, encrypted backend or RSS is claimed.

use super::super::super::super::lookup_membership::{MembershipLookupV1, scratch_bytes};
use super::super::super::super::lookup_sort::take_clear_observations;
use super::*;
use crate::plonk::lookup::prover::stored_membership_ordinary_oracle;
use crate::poly::commitment::ParamsProver;
use ff::{Field, PrimeField};
use std::sync::atomic::{AtomicBool, Ordering};

struct MembershipCircuit<C: CurveAffine, const FOUR: bool, const EMPTY: bool>(
    EmptyLookupProducer<C>,
);
impl<C: CurveAffine, const FOUR: bool, const EMPTY: bool> Circuit<C::Scalar>
    for MembershipCircuit<C, FOUR, EMPTY>
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
        let instances = std::array::from_fn::<_, 4, _>(|_| meta.instance_column());
        if FOUR {
            // Nine actual query points imply eleven blinding factors plus one final tail row.
            // A legitimate k=4 original key therefore has exactly four usable rows.
            let advice = meta.advice_column();
            meta.create_gate("nine rotation zero witness", |meta| {
                (0..9)
                    .map(|row| meta.query_advice(advice, Rotation(row)))
                    .collect::<Vec<_>>()
            });
        }
        if !EMPTY {
            for lookup in 0..2 {
                meta.lookup_any("independent input/table instance pair", |meta| {
                    vec![(
                        meta.query_instance(instances[2 * lookup], Rotation::cur()),
                        meta.query_instance(instances[2 * lookup + 1], Rotation::cur()),
                    )]
                });
            }
        }
        EmptyConfig {
            fixed,
            instance: instances[0],
        }
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

fn membership_key<C, const FOUR: bool, const EMPTY: bool>(params: &ParamsIPA<C>) -> ProvingKey<C>
where
    C: CurveAffine,
    C::Scalar: FromUniformBytes<64>,
{
    let shared = Shared::<C>::new();
    let producer =
        MembershipCircuit::<C, FOUR, EMPTY>(EmptyLookupProducer(Producer::new(&shared, 0)));
    let vk = keygen_vk_custom(params, &producer, true).unwrap();
    keygen_pk(params, vk, &producer).unwrap()
}

/// Independent dense test oracle: a multiset removes one occurrence per distinct input.
/// Production must never use this allocation strategy.
fn reference_leftovers<F: StoredAssignmentFieldV1 + Ord>(input: &[F], table: &[F]) -> (Vec<F>, Vec<F>) {
    let mut sorted = input.to_vec();
    sorted.sort();
    let unique = sorted
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let mut counts = std::collections::BTreeMap::<F, usize>::new();
    for value in table {
        *counts.entry(*value).or_default() += 1;
    }
    for value in unique {
        let count = counts.get_mut(&value).expect("valid fixture membership");
        assert!(*count > 0);
        *count -= 1;
    }
    let leftovers = counts
        .into_iter()
        .flat_map(|(value, count)| std::iter::repeat_n(value, count))
        .collect();
    (sorted, leftovers)
}

fn fixture_values<F: StoredAssignmentFieldV1 + Ord>(usable: usize, four: bool) -> [Vec<F>; 4] {
    let (input, table) = if four {
        assert_eq!(usable, 4);
        (
            [1, 1, 3, 3].map(F::from).to_vec(),
            [1, 2, 3, 4].map(F::from).to_vec(),
        )
    } else {
        let repertoire = [
            F::ZERO,
            F::ONE,
            F::from(3),
            F::from(256),
            F::from(257),
            F::from(65_536),
            -F::from(100),
        ];
        let input = (0..usable)
            .map(|i| repertoire[(i / 67) % repertoire.len()])
            .collect::<Vec<_>>();
        let mut table = input
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        while table.len() < usable {
            table.push(match table.len() % 5 {
                0 => F::from(2),
                1 => F::from(4),
                2 => F::ZERO,
                3 => F::from(256),
                _ => -F::from(100),
            });
        }
        table.reverse();
        (input, table)
    };
    [input.clone(), table.clone(), input, table]
}

fn ordinary_membership<C, const FOUR: bool>(k: u32)
where
    C: CurveAffine + crate::SerdeCurveAffine,
    C::Scalar: StoredAssignmentFieldV1
        + WithSmallOrderMulGroup<3>
        + FromUniformBytes<64>
        + crate::SerdePrimeField,
{
    let params = ParamsIPA::<C>::new(k);
    let pk = membership_key::<C, FOUR, false>(&params);
    let usable = (1 << k) - (pk.vk.cs.blinding_factors() + 1);
    let values = fixture_values::<C::Scalar>(usable, FOUR);
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let shared = Shared::<C>::new();
    let mut sorted =
        prepare_single_phase_stored_ipa_prefix_v1::<C, _, _, _, Challenge255<C>, _, false, 0>(
            &params,
            pk,
            MembershipCircuit::<C, FOUR, false>(EmptyLookupProducer(Producer::new(&shared, 0))),
            &instances,
            Provider::new(&shared),
            Rng(Arc::clone(&shared)),
            RecordingTranscript::new(&shared),
        )
        .unwrap()
        .stage_advice_coefficients()
        .unwrap()
        .compress_lookups(1 << 20)
        .unwrap()
        .sort_lookup_values(1 << 20)
        .unwrap();
    let originals = sorted
        .compressed
        .lookups
        .iter_mut()
        .map(|pair| {
            [
                read_compressed::<C, _>(&mut pair.input.snapshot, pair.input.layout),
                read_compressed::<C, _>(&mut pair.table.snapshot, pair.table.layout),
            ]
        })
        .collect::<Vec<_>>();
    let mut expected = Vec::new();
    for original in &originals {
        let (a, leftovers) = reference_leftovers(&original[0][..usable], &original[1][..usable]);
        let (ordinary_a, ordinary_s) = stored_membership_ordinary_oracle(
            &sorted.compressed.inner.pk,
            &params,
            original[0].clone(),
            original[1].clone(),
        );
        assert_eq!(a, ordinary_a[..usable]);
        let ordinary_leftovers = (1..usable)
            .filter(|i| ordinary_a[*i] == ordinary_a[*i - 1])
            .map(|i| ordinary_s[i])
            .collect::<Vec<_>>();
        assert_eq!(leftovers, ordinary_leftovers);
        if FOUR {
            assert_eq!(leftovers, [C::Scalar::from(2), C::Scalar::from(4)]);
        }
        expected.push((a, leftovers));
    }
    let lookup_ptr = sorted.compressed.lookups.as_ptr();
    let fixed_ptr = sorted.compressed.inner.pk.fixed_values.as_ptr();
    let vk_bytes = sorted
        .compressed
        .inner
        .pk
        .get_vk()
        .to_bytes(crate::SerdeFormat::Processed);
    let theta = *sorted.compressed.theta;
    let layouts = sorted
        .compressed
        .inner
        .advice
        .layouts()
        .unwrap()
        .collect::<Vec<_>>();
    let challenges = sorted
        .compressed
        .inner
        .advice
        .challenges()
        .unwrap()
        .collect::<Vec<_>>();
    let sorted_input_layouts = sorted
        .sorted
        .iter()
        .map(|p| p.input.layout)
        .collect::<Vec<_>>();
    let last_sorted = sorted.sorted.last().unwrap().table.layout.ordinal();
    let (events, draws, sealed) = {
        let log = shared.log.lock().unwrap();
        (log.events.clone(), log.rng_calls, log.sealed.clone())
    };
    let mut expected_rng = shared.rng.lock().unwrap().clone();
    take_clear_observations();
    let budget = scratch_bytes::<C::Scalar, Snapshot<C>>(2).unwrap();
    assert!(budget >= 32_768 + 2 * std::mem::size_of::<MembershipLookupV1<Snapshot<C>>>());
    let mut member = sorted.prepare_lookup_membership(budget).unwrap();
    assert_eq!(member.usable_rows, usable);
    assert_eq!(member.lookups.len(), 2);
    assert_eq!(member.compressed.lookups.as_ptr(), lookup_ptr);
    assert_eq!(member.compressed.inner.pk.fixed_values.as_ptr(), fixed_ptr);
    assert_eq!(
        member
            .compressed
            .inner
            .pk
            .get_vk()
            .to_bytes(crate::SerdeFormat::Processed),
        vk_bytes
    );
    assert_eq!(*member.compressed.theta, theta);
    assert!(std::ptr::eq(member.compressed.inner.params, &params));
    assert_eq!(
        member.compressed.inner.instances.as_ptr(),
        instances.as_ptr()
    );
    assert_eq!(
        member
            .compressed
            .inner
            .advice
            .layouts()
            .unwrap()
            .collect::<Vec<_>>(),
        layouts
    );
    assert_eq!(
        member
            .compressed
            .inner
            .advice
            .challenges()
            .unwrap()
            .collect::<Vec<_>>(),
        challenges
    );
    assert!(Arc::ptr_eq(
        &member.compressed.inner.provider.shared,
        &shared
    ));
    assert!(Arc::ptr_eq(&member.compressed.inner.rng.0, &shared));
    assert!(Arc::ptr_eq(
        &member.compressed.inner.transcript.shared,
        &shared
    ));
    for (index, pair) in member.lookups.iter_mut().enumerate() {
        assert_eq!(pair.input.layout, sorted_input_layouts[index]);
        let a = read_compressed::<C, _>(&mut pair.input.snapshot, pair.input.layout);
        let s = read_compressed::<C, _>(
            &mut pair.leftover_table.snapshot,
            pair.leftover_table.layout,
        );
        assert_eq!(&a[..usable], &expected[index].0);
        assert_eq!(pair.leftover_rows, expected[index].1.len());
        assert_eq!(pair.distinct_inputs + pair.leftover_rows, usable);
        assert_eq!(&s[..pair.leftover_rows], &expected[index].1);
        assert!(
            s[pair.leftover_rows..]
                .iter()
                .all(|v| *v == C::Scalar::ZERO)
        );
        assert!(a[usable..].iter().all(|v| *v == C::Scalar::ZERO));
        assert_eq!(
            pair.leftover_table.layout.role(),
            StoredPolynomialRoleV1::LookupLeftoverTable {
                lookup: index as u32
            }
        );
        assert_eq!(
            pair.leftover_table.layout.basis(),
            StoredPolynomialBasisV1::Lagrange
        );
        assert_eq!(
            pair.leftover_table.layout.ordinal(),
            last_sorted + 1 + index as u64
        );
        assert!(
            pair.leftover_table
                .layout
                .same_proof_context(pair.input.layout)
        );
    }
    for (index, pair) in member.compressed.lookups.iter_mut().enumerate() {
        assert_eq!(
            read_compressed::<C, _>(&mut pair.input.snapshot, pair.input.layout),
            originals[index][0]
        );
        assert_eq!(
            read_compressed::<C, _>(&mut pair.table.snapshot, pair.table.layout),
            originals[index][1]
        );
    }
    {
        let log = shared.log.lock().unwrap();
        assert_eq!(log.events, events);
        assert_eq!(log.rng_calls, draws);
        assert_eq!(&log.sealed[..sealed.len()], &sealed);
        assert_eq!(log.sealed.len(), sealed.len() + 2);
        assert_eq!(
            (log.provider_drops, log.rng_drops, log.transcript_drops),
            (0, 0, 0)
        );
    }
    let (fields, fields_zero, bytes, bytes_zero) = take_clear_observations();
    assert!(fields >= 3 * 256 && bytes >= 256 && fields_zero && bytes_zero);
    let (mut next, mut expected_next) = ([0; 64], [0; 64]);
    member.compressed.inner.rng.fill_bytes(&mut next);
    expected_rng.fill_bytes(&mut expected_next);
    assert_eq!(next, expected_next);
    drop(member);
    assert_dropped(&shared);
}

#[test]
fn eq_membership_exact_four_rows_and_cross_chunk_leftovers_match_active_ordinary_oracle() {
    ordinary_membership::<EqAffine, true>(4);
    for k in [4, 8, 9, 10] {
        ordinary_membership::<EqAffine, false>(k);
    }
}
#[test]
fn ep_membership_exact_four_rows_and_cross_chunk_leftovers_match_active_ordinary_oracle() {
    ordinary_membership::<EpAffine, true>(4);
    for k in [4, 8, 9, 10] {
        ordinary_membership::<EpAffine, false>(k);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MemberChange {
    Context,
    Ordinal,
    OrdinalExhausted,
    Lookup,
    SortedRole,
    CompressedRole,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MemberFault {
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
    Capacity(usize),
    CreateLayout(MemberChange),
    WriterSecondObservation,
    WriterAfterWrite,
    SnapshotLayout(u64),
    AfterSeal(u64, u64),
    AfterRead(u64, u64, u64),
}
fn member_change(
    layout: StoredPolynomialLayoutV1,
    change: MemberChange,
) -> StoredPolynomialLayoutV1 {
    let lookup = match layout.role() {
        StoredPolynomialRoleV1::LookupLeftoverTable { lookup } => lookup,
        _ => panic!("expected leftover writer"),
    };
    StoredPolynomialLayoutV1::new(
        if change == MemberChange::Context {
            [41; 32]
        } else {
            [23; 32]
        },
        if change == MemberChange::Ordinal {
            layout.ordinal() - 1
        } else if change == MemberChange::OrdinalExhausted {
            u64::MAX
        } else {
            layout.ordinal()
        },
        layout.field(),
        layout.basis(),
        layout.k(),
        match change {
            MemberChange::Lookup => {
                StoredPolynomialRoleV1::LookupLeftoverTable { lookup: lookup + 1 }
            }
            MemberChange::SortedRole => StoredPolynomialRoleV1::LookupSorted {
                lookup,
                side: StoredLookupSideV1::Table,
                run_log: layout.k(),
            },
            MemberChange::CompressedRole => StoredPolynomialRoleV1::LookupCompressed {
                lookup,
                side: StoredLookupSideV1::Table,
            },
            _ => layout.role(),
        },
    )
    .unwrap()
}
struct MemberProvider<C: CurveAffine> {
    inner: Provider<C>,
    fault: Arc<Mutex<MemberFault>>,
    window: Arc<AtomicBool>,
}
struct MemberWriter<C: CurveAffine> {
    inner: Writer<C>,
    fault: Arc<Mutex<MemberFault>>,
    window: Arc<AtomicBool>,
    observations: std::cell::Cell<usize>,
}
struct MemberSnapshot<C: CurveAffine> {
    inner: Snapshot<C>,
    fault: Arc<Mutex<MemberFault>>,
    window: Arc<AtomicBool>,
}
impl<C: CurveAffine> StoredPolynomialProviderV1 for MemberProvider<C> {
    type Writer = MemberWriter<C>;
    fn create(
        &mut self,
        field: StoredPastaFieldV1,
        basis: StoredPolynomialBasisV1,
        k: u32,
        role: StoredPolynomialRoleV1,
    ) -> Result<Self::Writer, StoredPolynomialErrorV1> {
        assert!(!self.window.load(Ordering::SeqCst));
        let fault = *self.fault.lock().unwrap();
        if fault == MemberFault::Create(self.inner.ordinal) {
            return Err(StoredPolynomialErrorV1::Storage);
        }
        if let MemberFault::Capacity(cap) = fault {
            let log = self.inner.shared.log.lock().unwrap();
            if log.sealed.len() - log.snapshot_drops >= cap {
                return Err(StoredPolynomialErrorV1::Capacity);
            }
        }
        let mut inner = self.inner.create(field, basis, k, role)?;
        if matches!(role, StoredPolynomialRoleV1::LookupLeftoverTable { .. }) {
            if let MemberFault::CreateLayout(change) = fault {
                inner.layout = member_change(inner.layout, change);
            }
        }
        Ok(MemberWriter {
            inner,
            fault: Arc::clone(&self.fault),
            window: Arc::clone(&self.window),
            observations: std::cell::Cell::new(0),
        })
    }
}
impl<C: CurveAffine> StoredPolynomialWriterV1 for MemberWriter<C> {
    type Snapshot = MemberSnapshot<C>;
    fn layout(&self) -> StoredPolynomialLayoutV1 {
        let observation = self.observations.get();
        self.observations.set(observation + 1);
        let fault = *self.fault.lock().unwrap();
        if matches!(
            self.inner.layout.role(),
            StoredPolynomialRoleV1::LookupLeftoverTable { .. }
        ) && ((fault == MemberFault::WriterSecondObservation && observation > 0)
            || (fault == MemberFault::WriterAfterWrite && self.inner.next > 0))
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
        assert!(!self.window.load(Ordering::SeqCst));
        let fault = *self.fault.lock().unwrap();
        let ordinal = self.inner.layout.ordinal();
        assert_ne!(fault, MemberFault::PanicWrite(ordinal, chunk));
        if fault == MemberFault::Write(ordinal, chunk) {
            return Err(StoredPolynomialErrorV1::Storage);
        }
        self.inner.write_chunk(chunk, values)
    }
    fn seal(self) -> Result<Self::Snapshot, StoredPolynomialErrorV1> {
        assert!(!self.window.load(Ordering::SeqCst));
        let fault = *self.fault.lock().unwrap();
        let ordinal = self.inner.layout.ordinal();
        assert_ne!(fault, MemberFault::PanicSeal(ordinal));
        if fault == MemberFault::Seal(ordinal) {
            return Err(StoredPolynomialErrorV1::Storage);
        }
        let inner = self.inner.seal()?;
        if let MemberFault::AfterSeal(trigger, victim) = fault {
            if ordinal == trigger {
                override_receipt(&inner.shared, victim);
            }
        }
        Ok(MemberSnapshot {
            inner,
            fault: self.fault,
            window: self.window,
        })
    }
}
fn override_receipt<C: CurveAffine>(shared: &Arc<Shared<C>>, victim: u64) {
    let mut log = shared.log.lock().unwrap();
    let original = log
        .sealed
        .iter()
        .find(|(layout, _)| layout.ordinal() == victim)
        .unwrap()
        .0;
    log.snapshot_layout_override = Some((victim, different_context(original)));
}
impl<C: CurveAffine> StoredPolynomialSnapshotV1 for MemberSnapshot<C> {
    fn layout(&self) -> StoredPolynomialLayoutV1 {
        if *self.fault.lock().unwrap() == MemberFault::SnapshotLayout(self.inner.layout.ordinal()) {
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
        let fault = *self.fault.lock().unwrap();
        let ordinal = self.inner.layout.ordinal();
        assert!(!self.window.swap(true, Ordering::SeqCst));
        let _window = ReadWindow(Arc::clone(&self.window));
        if fault == MemberFault::Read(ordinal, chunk) {
            return Err(StoredPolynomialErrorV1::Storage);
        }
        let result = self.inner.with_chunk(expected, chunk, |values| {
            assert_ne!(fault, MemberFault::PanicRead(ordinal, chunk));
            if matches!(fault, MemberFault::Encoding(o, c) | MemberFault::ShortChunk(o, c) if o == ordinal && c == chunk) {
                let mut changed = values.to_vec();
                if fault == MemberFault::Encoding(ordinal, chunk) { changed[0] = [0xff; 32]; } else { changed.pop(); }
                consume(&changed)
            } else { consume(values) }
        });
        if let MemberFault::AfterRead(trigger, target_chunk, victim) = fault {
            if ordinal == trigger && chunk == target_chunk {
                override_receipt(&self.inner.shared, victim);
            }
        }
        result
    }
    fn with_column<V>(
        &mut self,
        _: StoredPolynomialLayoutV1,
        _: impl FnOnce(&[[u8; 32]]) -> Result<V, StoredPolynomialErrorV1>,
    ) -> Result<V, StoredPolynomialErrorV1> {
        panic!("membership requested unbounded full-column access")
    }
}

fn membership_faults<C>(metadata: bool)
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(9);
    let pk = membership_key::<C, false, false>(&params);
    let usable = 512 - (pk.vk.cs.blinding_factors() + 1);
    let values = fixture_values::<C::Scalar>(usable, false);
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    // Derive ordinals from the actual emitted banks, never from assumed phase geometry.
    for case in 0..if metadata { 22 } else { 13 } {
        let shared = Shared::<C>::new();
        let control = Arc::new(Mutex::new(MemberFault::None));
        let window = Arc::new(AtomicBool::new(false));
        let sorted =
            prepare_single_phase_stored_ipa_prefix_v1::<C, _, _, _, Challenge255<C>, _, false, 0>(
                &params,
                pk.clone(),
                MembershipCircuit::<C, false, false>(EmptyLookupProducer(Producer::new(
                    &shared, 0,
                ))),
                &instances,
                MemberProvider {
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
            .unwrap()
            .sort_lookup_values(1 << 20)
            .unwrap();
        let a = sorted.sorted[0].input.layout.ordinal();
        let s = sorted.sorted[0].table.layout.ordinal();
        let a2 = sorted.sorted[1].input.layout.ordinal();
        let s2 = sorted.sorted[1].table.layout.ordinal();
        let original = sorted.compressed.lookups[0].input.layout.ordinal();
        let output = sorted.compressed.inner.provider.inner.ordinal;
        let live = {
            let log = shared.log.lock().unwrap();
            log.sealed.len() - log.snapshot_drops
        };
        let fault = if metadata {
            match case {
                0 => MemberFault::CreateLayout(MemberChange::Context),
                1 => MemberFault::CreateLayout(MemberChange::Ordinal),
                2 => MemberFault::CreateLayout(MemberChange::Lookup),
                3 => MemberFault::CreateLayout(MemberChange::SortedRole),
                4 => MemberFault::CreateLayout(MemberChange::CompressedRole),
                5 => MemberFault::WriterSecondObservation,
                6 => MemberFault::WriterAfterWrite,
                7 => MemberFault::SnapshotLayout(a),
                8 => MemberFault::SnapshotLayout(s2),
                9 => MemberFault::AfterRead(a, 0, original),
                10 => MemberFault::AfterRead(s, 1, a2),
                11 => MemberFault::AfterRead(a2, 0, output),
                12 => MemberFault::AfterRead(a2, 0, a),
                13 => MemberFault::AfterRead(s2, 1, s2),
                14 => MemberFault::AfterSeal(output, original),
                15 => MemberFault::AfterSeal(output, a),
                16 => MemberFault::AfterSeal(output, s),
                17 => MemberFault::AfterSeal(output, s2),
                18 => MemberFault::AfterSeal(output + 1, output),
                19 => MemberFault::AfterSeal(output + 1, a),
                20 => MemberFault::AfterSeal(output + 1, output + 1),
                21 => MemberFault::CreateLayout(MemberChange::OrdinalExhausted),
                _ => unreachable!(),
            }
        } else {
            match case {
                0 => MemberFault::Create(output + 1),
                1 => MemberFault::Write(output, 1),
                2 => MemberFault::Seal(output + 1),
                3 => MemberFault::Read(a, 1),
                4 => MemberFault::Read(s2, 1),
                5 => MemberFault::PanicWrite(output + 1, 1),
                6 => MemberFault::PanicSeal(output + 1),
                7 => MemberFault::PanicRead(a2, 1),
                8 => MemberFault::Encoding(a, 1),
                9 => MemberFault::Encoding(s2, 1),
                10 => MemberFault::ShortChunk(a, 1),
                11 => MemberFault::ShortChunk(s2, 1),
                12 => MemberFault::Capacity(live),
                _ => unreachable!(),
            }
        };
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
        take_clear_observations();
        *control.lock().unwrap() = fault;
        let result = catch_unwind(AssertUnwindSafe(|| {
            sorted
                .prepare_lookup_membership(
                    scratch_bytes::<C::Scalar, MemberSnapshot<C>>(2).unwrap(),
                )
                .map(|_| ())
        }));
        if matches!(
            fault,
            MemberFault::PanicWrite(..) | MemberFault::PanicSeal(..) | MemberFault::PanicRead(..)
        ) {
            assert!(result.is_err(), "unreached unwind {fault:?}");
        } else {
            let error = result
                .unwrap()
                .expect_err("admitted injected membership fault");
            if matches!(fault, MemberFault::Capacity(_)) {
                assert!(
                    matches!(
                        error,
                        StoredLookupErrorV1::Phase(StoredPhaseErrorV1::Store(
                            StoredPolynomialErrorV1::Capacity
                        )) | StoredLookupErrorV1::Store(StoredPolynomialErrorV1::Capacity)
                    ),
                    "{error:?}"
                );
            }
        }
        assert!(!window.load(Ordering::SeqCst));
        assert_dropped(&shared);
        let log = shared.log.lock().unwrap();
        assert_eq!(log.events, events, "{fault:?} changed transcript");
        assert_eq!(log.rng_calls, draws, "{fault:?} drew proof randomness");
        assert_eq!(&log.sealed[..sealed.len()], &sealed);
        if matches!(
            fault,
            MemberFault::CreateLayout(_)
                | MemberFault::WriterSecondObservation
                | MemberFault::SnapshotLayout(_)
                | MemberFault::Capacity(_)
        ) {
            assert_eq!(log.reads, reads, "{fault:?} read witness before admission");
            assert_eq!(log.writes, writes);
        }
        let (_, fields_zero, _, bytes_zero) = take_clear_observations();
        assert!(
            fields_zero && bytes_zero,
            "{fault:?} left initialized tile contents"
        );
    }
}
#[test]
fn both_pasta_membership_retained_current_remaining_and_completed_receipt_substitutions_fail_closed()
 {
    membership_faults::<EqAffine>(true);
    membership_faults::<EpAffine>(true);
}
#[test]
fn both_pasta_membership_partial_backend_errors_unwind_and_capacity_refusal_destroy_all_owners() {
    membership_faults::<EqAffine>(false);
    membership_faults::<EpAffine>(false);
}

fn membership_edges<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(9);
    let pk = membership_key::<C, false, false>(&params);
    let usable = 512 - (pk.vk.cs.blinding_factors() + 1);
    for case in 0..7 {
        let mut values = fixture_values::<C::Scalar>(usable, false);
        match case {
            0 => {
                for lookup in 0..2 {
                    values[2 * lookup] = (1..=usable).map(|v| C::Scalar::from(v as u64)).collect();
                    values[2 * lookup + 1] = values[2 * lookup].iter().rev().copied().collect();
                }
            }
            1 => values = std::array::from_fn(|_| vec![C::Scalar::ZERO; usable]),
            2 => {
                values[2] = vec![C::Scalar::ZERO; usable];
                values[3] = vec![C::Scalar::ONE; usable];
            }
            3 => values[2][usable - 1] = -C::Scalar::ONE,
            5 => {
                values[2] = vec![C::Scalar::ZERO; usable];
                values[3] = (0..usable).map(|v| C::Scalar::from(v as u64)).collect();
            }
            _ => {}
        }
        let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let shared = Shared::<C>::new();
        let mut sorted =
            prepare_single_phase_stored_ipa_prefix_v1::<C, _, _, _, Challenge255<C>, _, false, 0>(
                &params,
                pk.clone(),
                MembershipCircuit::<C, false, false>(EmptyLookupProducer(Producer::new(
                    &shared, 0,
                ))),
                &instances,
                Provider::new(&shared),
                Rng(Arc::clone(&shared)),
                RecordingTranscript::new(&shared),
            )
            .unwrap()
            .stage_advice_coefficients()
            .unwrap()
            .compress_lookups(1 << 20)
            .unwrap()
            .sort_lookup_values(1 << 20)
            .unwrap();
        if case == 4 {
            sorted.sorted[1].input.snapshot.values.swap(0, usable - 1);
        }
        if case == 5 {
            sorted.sorted[1]
                .table
                .snapshot
                .values
                .swap(usable - 2, usable - 1);
        }
        if case == 6 {
            // The private row count excludes all padded slots, independent of their values.
            for pair in &mut sorted.sorted {
                for column in [&mut pair.input, &mut pair.table] {
                    column.snapshot.values[usable..].fill((-C::Scalar::ONE).to_repr());
                }
            }
        }
        let (events, draws, sealed) = {
            let log = shared.log.lock().unwrap();
            (log.events.clone(), log.rng_calls, log.sealed.len())
        };
        take_clear_observations();
        let result =
            sorted.prepare_lookup_membership(scratch_bytes::<C::Scalar, Snapshot<C>>(2).unwrap());
        if (2..=5).contains(&case) {
            let error = result.err().expect("invalid active population accepted");
            if case <= 3 {
                assert_eq!(error, StoredLookupErrorV1::Membership);
            } else {
                assert_eq!(error, StoredLookupErrorV1::Context);
            }
            // Lookup0 completed and its leftover bank was sealed before lookup1 failed.
            assert_eq!(shared.log.lock().unwrap().sealed.len(), sealed + 1);
        } else {
            let mut member = result.unwrap();
            for (lookup, pair) in member.lookups.iter_mut().enumerate() {
                let (_, expected) =
                    reference_leftovers(&values[2 * lookup], &values[2 * lookup + 1]);
                assert_eq!(pair.leftover_rows, expected.len());
                assert_eq!(pair.distinct_inputs, usable - expected.len());
                let actual = read_compressed::<C, _>(
                    &mut pair.leftover_table.snapshot,
                    pair.leftover_table.layout,
                );
                assert_eq!(&actual[..expected.len()], &expected);
                assert!(
                    actual[expected.len()..]
                        .iter()
                        .all(|v| *v == C::Scalar::ZERO)
                );
                if case == 0 {
                    assert_eq!(pair.leftover_rows, 0);
                }
                if case == 1 {
                    assert_eq!(pair.leftover_rows, usable - 1);
                    assert_eq!(pair.distinct_inputs, 1);
                }
            }
            drop(member);
        }
        assert_dropped(&shared);
        let log = shared.log.lock().unwrap();
        assert_eq!(log.events, events);
        assert_eq!(log.rng_calls, draws);
        let (fields, zero_fields, bytes, zero_bytes) = take_clear_observations();
        assert!(fields >= 3 * 256 && bytes >= 256 && zero_fields && zero_bytes);
    }
}
#[test]
fn both_pasta_membership_counts_exclude_padding_and_late_missing_or_unsorted_population_destroys_completed_banks()
 {
    membership_edges::<EqAffine>();
    membership_edges::<EpAffine>();
}

fn membership_preflights<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(4);
    let wrong_params = ParamsIPA::<C>::new(5);
    let pk = membership_key::<C, true, false>(&params);
    let values = fixture_values::<C::Scalar>(4, true);
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    for case in 0..10 {
        let shared = Shared::<C>::new();
        let mut sorted =
            prepare_single_phase_stored_ipa_prefix_v1::<C, _, _, _, Challenge255<C>, _, false, 0>(
                &params,
                pk.clone(),
                MembershipCircuit::<C, true, false>(EmptyLookupProducer(Producer::new(&shared, 0))),
                &instances,
                Provider::new(&shared),
                Rng(Arc::clone(&shared)),
                RecordingTranscript::new(&shared),
            )
            .unwrap()
            .stage_advice_coefficients()
            .unwrap()
            .compress_lookups(1 << 20)
            .unwrap()
            .sort_lookup_values(1 << 20)
            .unwrap();
        let mut budget = scratch_bytes::<C::Scalar, Snapshot<C>>(2).unwrap();
        match case {
            0 => budget = 0,
            1 => budget -= 1,
            2 => {
                sorted.sorted.pop();
            }
            3 => {
                let pair = &mut sorted.sorted[0];
                std::mem::swap(&mut pair.input, &mut pair.table);
            }
            4 => sorted.compressed.inner.params = &wrong_params,
            5 => sorted.usable_rows += 1,
            6 => {
                let column = &mut sorted.sorted[1].table;
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
            7 => override_receipt(&shared, sorted.compressed.lookups[0].input.layout.ordinal()),
            8 => {
                let ordinal = sorted
                    .compressed
                    .inner
                    .advice
                    .layouts()
                    .unwrap()
                    .next()
                    .unwrap()
                    .ordinal();
                override_receipt(&shared, ordinal);
            }
            9 => {
                sorted.compressed.lookups.pop();
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
        let result = sorted.prepare_lookup_membership(budget).map(|_| ());
        if case <= 1 {
            assert_eq!(result, Err(StoredLookupErrorV1::ScratchLimit));
        } else {
            assert!(result.is_err(), "preflight {case} admitted");
        }
        assert_dropped(&shared);
        let log = shared.log.lock().unwrap();
        assert_eq!(log.events, events);
        assert_eq!(log.rng_calls, draws);
        assert_eq!(log.reads, reads, "preflight {case} reached witness bytes");
        assert_eq!(log.writes, writes);
        assert_eq!(log.created, created);
        assert_eq!(take_clear_observations(), (0, true, 0, true));
    }
}
#[test]
fn both_pasta_membership_budget_original_key_geometry_and_ordinal_overflow_fail_before_io() {
    membership_preflights::<EqAffine>();
    membership_preflights::<EpAffine>();
}

fn empty_membership<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(4);
    let pk = membership_key::<C, true, true>(&params);
    let values = fixture_values::<C::Scalar>(4, true);
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let shared = Shared::<C>::new();
    let sorted =
        prepare_single_phase_stored_ipa_prefix_v1::<C, _, _, _, Challenge255<C>, _, false, 0>(
            &params,
            pk,
            MembershipCircuit::<C, true, true>(EmptyLookupProducer(Producer::new(&shared, 0))),
            &instances,
            Provider::new(&shared),
            Rng(Arc::clone(&shared)),
            RecordingTranscript::new(&shared),
        )
        .unwrap()
        .stage_advice_coefficients()
        .unwrap()
        .compress_lookups(0)
        .unwrap()
        .sort_lookup_values(0)
        .unwrap();
    let (events, draws, reads, writes, created, sealed) = {
        let log = shared.log.lock().unwrap();
        (
            log.events.clone(),
            log.rng_calls,
            log.reads,
            log.writes,
            log.created,
            log.sealed.clone(),
        )
    };
    let advice = sorted
        .compressed
        .inner
        .advice
        .layouts()
        .unwrap()
        .collect::<Vec<_>>();
    assert_eq!(advice.len(), 1);
    assert_eq!(scratch_bytes::<C::Scalar, Snapshot<C>>(0).unwrap(), 0);
    assert_eq!(
        scratch_bytes::<C::Scalar, Snapshot<C>>(usize::MAX),
        Err(StoredLookupErrorV1::Context)
    );
    take_clear_observations();
    let member = sorted.prepare_lookup_membership(0).unwrap();
    assert!(member.lookups.is_empty());
    assert!(member.compressed.lookups.is_empty());
    assert_eq!(
        member
            .compressed
            .inner
            .advice
            .layouts()
            .unwrap()
            .collect::<Vec<_>>(),
        advice
    );
    assert_eq!(take_clear_observations(), (0, true, 0, true));
    {
        let log = shared.log.lock().unwrap();
        assert_eq!(log.events, events);
        assert_eq!(log.rng_calls, draws);
        assert_eq!(log.reads, reads);
        assert_eq!(log.writes, writes);
        assert_eq!(log.created, created);
        assert_eq!(log.sealed, sealed);
    }
    drop(member);
    assert_dropped(&shared);
}
#[test]
fn both_pasta_zero_lookups_keep_existing_advice_without_scratch_rng_transcript_or_store_activity() {
    empty_membership::<EqAffine>();
    empty_membership::<EpAffine>();
}

#[path = "lookup_permuted_tests.rs"]
mod permuted;
