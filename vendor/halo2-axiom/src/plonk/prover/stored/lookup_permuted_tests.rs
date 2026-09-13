//! Independent active ordinary-prover parity and consuming-owner fault regressions.
//!
//! These are plaintext fixture banks. They do not qualify encrypted Core storage, a complete
//! stored proof, cryptographic admission of a detached key, or process RSS. The only dense
//! permutation oracle calls the active ordinary `Argument::commit_permuted` implementation.

use super::*;
use crate::plonk::lookup::prover::stored_permuted_ordinary_oracle;
use crate::plonk::prover::stored::lookup_permuted::{
    scratch_bytes as permuted_scratch_bytes, take_clear_observations as take_permuted_clears,
};

fn replay_prefix<C>(events: &[Event<C>], transcript: &mut RecordingTranscript<C>)
where
    C: CurveAffine,
    C::Scalar: FromUniformBytes<64>,
{
    for event in events {
        match *event {
            Event::CommonScalar(value) => transcript.common_scalar(value).unwrap(),
            Event::CommonPoint(value) => transcript.common_point(value).unwrap(),
            Event::WriteScalar(value) => transcript.write_scalar(value).unwrap(),
            Event::WritePoint(value) => transcript.write_point(value).unwrap(),
            Event::Challenge(value) => {
                assert_eq!(transcript.squeeze_challenge().get_scalar(), value);
            }
        }
    }
}

fn ordinary_permuted<C, const FOUR: bool, const Q: bool, const M: u64>(k: u32)
where
    C: CurveAffine + crate::SerdeCurveAffine,
    C::Scalar: StoredAssignmentFieldV1
        + WithSmallOrderMulGroup<3>
        + FromUniformBytes<64>
        + crate::SerdePrimeField,
{
    let params = ParamsIPA::<C>::new(k);
    let pk = membership_key::<C, FOUR, false>(&params);
    let n = 1_usize << k;
    let usable = n - (pk.vk.cs.blinding_factors() + 1);
    let values = fixture_values::<C::Scalar>(usable, FOUR);
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let shared = Shared::<C>::new();
    let mut member =
        prepare_single_phase_stored_ipa_prefix_v1::<C, _, _, _, Challenge255<C>, _, Q, M>(
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
        .unwrap()
        .prepare_lookup_membership(1 << 20)
        .unwrap();
    let original_compressed = member
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
    let retained_advice = member
        .compressed
        .inner
        .advice
        .layouts()
        .unwrap()
        .collect::<Vec<_>>();
    let challenges = member
        .compressed
        .inner
        .advice
        .challenges()
        .unwrap()
        .collect::<Vec<_>>();
    let (prefix, sealed, draws) = {
        let log = shared.log.lock().unwrap();
        (log.events.clone(), log.sealed.clone(), log.rng_calls)
    };
    // The exact original admitted advice values, including their already-drawn tails, are
    // read from the test backend. This does not synthesize a second witness or reseed RNG.
    let advice = retained_advice
        .iter()
        .map(|layout| {
            let encoded = &sealed
                .iter()
                .find(|(identity, _)| identity == layout)
                .unwrap()
                .1;
            member.compressed.inner.pk.vk.domain.lagrange_from_vec(
                encoded
                    .iter()
                    .map(|bytes| Option::<C::Scalar>::from(C::Scalar::from_repr(*bytes)).unwrap())
                    .collect(),
            )
        })
        .collect::<Vec<_>>();
    let dense_instances = values
        .iter()
        .map(|column| {
            let mut full = column.clone();
            full.resize(n, C::Scalar::ZERO);
            member.compressed.inner.pk.vk.domain.lagrange_from_vec(full)
        })
        .collect::<Vec<_>>();
    let ordinary = Shared::<C>::new();
    let mut oracle_transcript = RecordingTranscript::new(&ordinary);
    replay_prefix(&prefix, &mut oracle_transcript);
    let mut oracle_rng = shared.rng.lock().unwrap().clone();
    let mut expected = Vec::new();
    for lookup in 0..member.lookups.len() {
        expected.push(
            stored_permuted_ordinary_oracle(
                &member.compressed.inner.pk,
                &params,
                lookup,
                member.compressed.theta,
                &advice,
                &dense_instances,
                &challenges,
                &mut oracle_rng,
                &mut oracle_transcript,
            )
            .unwrap(),
        );
    }
    let lookup_ptr = member.compressed.lookups.as_ptr();
    let fixed_ptr = member.compressed.inner.pk.fixed_values.as_ptr();
    let vk_bytes = member
        .compressed
        .inner
        .pk
        .get_vk()
        .to_bytes(crate::SerdeFormat::Processed);
    let theta = *member.compressed.theta;
    let first_output = member.compressed.inner.provider.ordinal;
    let budget = permuted_scratch_bytes::<C, Snapshot<C>>(k, 2).unwrap();
    assert!(budget >= n * std::mem::size_of::<C::Scalar>());
    take_permuted_clears();
    let mut permuted = member.commit_permuted_lookups(budget).unwrap();
    assert_eq!(permuted.usable_rows, usable);
    assert_eq!(permuted.lookups.len(), expected.len());
    assert_eq!(permuted.compressed.lookups.as_ptr(), lookup_ptr);
    assert_eq!(
        permuted.compressed.inner.pk.fixed_values.as_ptr(),
        fixed_ptr
    );
    assert_eq!(
        permuted
            .compressed
            .inner
            .pk
            .get_vk()
            .to_bytes(crate::SerdeFormat::Processed),
        vk_bytes
    );
    assert_eq!(*permuted.compressed.theta, theta);
    assert!(std::ptr::eq(permuted.compressed.inner.params, &params));
    assert_eq!(
        permuted.compressed.inner.instances.as_ptr(),
        instances.as_ptr()
    );
    assert!(Arc::ptr_eq(
        &permuted.compressed.inner.provider.shared,
        &shared
    ));
    assert!(Arc::ptr_eq(&permuted.compressed.inner.rng.0, &shared));
    assert!(Arc::ptr_eq(
        &permuted.compressed.inner.transcript.shared,
        &shared
    ));
    assert_eq!(
        permuted
            .compressed
            .inner
            .advice
            .layouts()
            .unwrap()
            .collect::<Vec<_>>(),
        retained_advice
    );
    assert_eq!(
        permuted
            .compressed
            .inner
            .advice
            .challenges()
            .unwrap()
            .collect::<Vec<_>>(),
        challenges
    );
    let expected_points = ordinary.log.lock().unwrap().events[prefix.len()..]
        .iter()
        .map(|event| match event {
            Event::WritePoint(point) => *point,
            _ => panic!("ordinary lookup emitted non-point"),
        })
        .collect::<Vec<_>>();
    assert_eq!(expected_points.len(), 4);
    let mut ordinals = Vec::new();
    for (lookup, (pair, oracle)) in permuted.lookups.iter_mut().zip(&expected).enumerate() {
        for (side_index, (side, column, lagrange, coefficient, blind)) in [
            (
                StoredLookupSideV1::Input,
                &mut pair.input,
                &oracle.input_lagrange,
                &oracle.input_coefficient,
                oracle.input_blind,
            ),
            (
                StoredLookupSideV1::Table,
                &mut pair.table,
                &oracle.table_lagrange,
                &oracle.table_coefficient,
                oracle.table_blind,
            ),
        ]
        .into_iter()
        .enumerate()
        {
            // These comparisons include every usable and random tail row, not just membership.
            let actual_lagrange =
                read_compressed::<C, _>(&mut column.lagrange.snapshot, column.lagrange.layout);
            assert_eq!(&actual_lagrange, lagrange);
            assert_eq!(actual_lagrange.len(), n);
            assert!(
                actual_lagrange[usable..]
                    .iter()
                    .any(|v| *v != C::Scalar::ZERO)
            );
            assert_eq!(
                read_compressed::<C, _>(
                    &mut column.coefficient.snapshot,
                    column.coefficient.layout
                ),
                *coefficient
            );
            assert_eq!((column.blind.0).0, blind.0);
            assert_eq!(column.commitment, expected_points[2 * lookup + side_index]);
            let point = params
                .commit_lagrange(
                    &permuted
                        .compressed
                        .inner
                        .pk
                        .vk
                        .domain
                        .lagrange_from_vec(actual_lagrange),
                    blind,
                )
                .to_affine();
            assert_eq!(point, column.commitment);
            for (polynomial, basis) in [
                (&column.lagrange, StoredPolynomialBasisV1::Lagrange),
                (&column.coefficient, StoredPolynomialBasisV1::Coefficient),
            ] {
                assert_eq!(
                    polynomial.layout.role(),
                    StoredPolynomialRoleV1::LookupPermuted {
                        lookup: lookup as u32,
                        side
                    }
                );
                assert_eq!(polynomial.layout.basis(), basis);
                assert_eq!(polynomial.layout.k(), k);
                assert!(polynomial.layout.same_proof_context(column.lagrange.layout));
                assert_eq!(polynomial.snapshot.layout(), polynomial.layout);
                ordinals.push(polynomial.layout.ordinal());
            }
        }
        if FOUR {
            assert_eq!(
                &oracle.input_lagrange[..usable],
                &[1, 1, 3, 3].map(C::Scalar::from)
            );
            assert_eq!(
                &oracle.table_lagrange[..usable],
                &[1, 2, 3, 4].map(C::Scalar::from)
            );
        }
    }
    ordinals.sort_unstable();
    assert_eq!(
        ordinals,
        (first_output..first_output + 8).collect::<Vec<_>>()
    );
    for (index, pair) in permuted.compressed.lookups.iter_mut().enumerate() {
        assert_eq!(
            read_compressed::<C, _>(&mut pair.input.snapshot, pair.input.layout),
            original_compressed[index][0]
        );
        assert_eq!(
            read_compressed::<C, _>(&mut pair.table.snapshot, pair.table.layout),
            original_compressed[index][1]
        );
    }
    {
        let log = shared.log.lock().unwrap();
        assert_eq!(&log.sealed[..sealed.len()], &sealed);
        assert_eq!(log.sealed.len(), sealed.len() + 8);
        assert_eq!(log.events, ordinary.log.lock().unwrap().events);
        assert!(log.rng_calls > draws);
        assert_eq!(
            (log.provider_drops, log.rng_drops, log.transcript_drops),
            (0, 0, 0)
        );
    }
    let (mut next, mut expected_next) = ([0; 64], [0; 64]);
    permuted.compressed.inner.rng.fill_bytes(&mut next);
    oracle_rng.fill_bytes(&mut expected_next);
    assert_eq!(
        next, expected_next,
        "tail/blind RNG ordering or consumption changed"
    );
    assert_eq!(
        permuted
            .compressed
            .inner
            .transcript
            .squeeze_challenge()
            .get_scalar(),
        oracle_transcript.squeeze_challenge().get_scalar()
    );
    assert_eq!(
        shared.log.lock().unwrap().events,
        ordinary.log.lock().unwrap().events
    );
    let stored_bytes = std::mem::replace(
        &mut permuted.compressed.inner.transcript.inner,
        Blake2bWrite::init(Vec::new()),
    )
    .finalize();
    let oracle_bytes =
        std::mem::replace(&mut oracle_transcript.inner, Blake2bWrite::init(Vec::new())).finalize();
    assert_eq!(
        stored_bytes, oracle_bytes,
        "complete transcript serialization changed"
    );
    drop(permuted);
    let (fields, fields_zero, encoded, bytes_zero, blinds, blinds_zero) = take_permuted_clears();
    assert!(fields >= n && encoded >= 256 && blinds == 4);
    assert!(fields_zero && bytes_zero && blinds_zero);
    assert_dropped(&shared);
}

#[test]
fn eq_permuted_pair_coefficients_blinds_points_transcript_and_rng_match_active_ordinary() {
    ordinary_permuted::<EqAffine, true, false, 0>(4);
    for k in [4, 8, 9] {
        ordinary_permuted::<EqAffine, false, false, 0>(k);
        ordinary_permuted::<EqAffine, false, true, 0>(k);
        ordinary_permuted::<EqAffine, false, true, 6>(k);
    }
}

#[test]
fn ep_permuted_pair_coefficients_blinds_points_transcript_and_rng_match_active_ordinary() {
    ordinary_permuted::<EpAffine, true, false, 0>(4);
    for k in [4, 8, 9] {
        ordinary_permuted::<EpAffine, false, false, 0>(k);
        ordinary_permuted::<EpAffine, false, true, 0>(k);
        ordinary_permuted::<EpAffine, false, true, 6>(k);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PermutedChange {
    Context,
    Ordinal,
    Exhausted,
    InsufficientOrdinals,
    Field,
    Basis,
    K,
    Lookup,
    Side,
    CompressedRole,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PermutedFault {
    None,
    CreateLayout(PermutedChange),
    WriterSecondObservation,
    WriterAfterWrite,
    ReadOccurrence {
        ordinal: u64,
        chunk: u64,
        occurrence: usize,
        panic: bool,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BoundaryFault {
    None,
    Rng(usize),
    Transcript(usize),
}
#[derive(Clone)]
struct Controls {
    backend: Arc<Mutex<MemberFault>>,
    metadata: Arc<Mutex<PermutedFault>>,
    boundary: Arc<Mutex<BoundaryFault>>,
    window: Arc<AtomicBool>,
    reads: Arc<Mutex<std::collections::BTreeMap<(u64, u64), usize>>>,
}
impl Controls {
    fn new() -> Self {
        Self {
            backend: Arc::new(Mutex::new(MemberFault::None)),
            metadata: Arc::new(Mutex::new(PermutedFault::None)),
            boundary: Arc::new(Mutex::new(BoundaryFault::None)),
            window: Arc::new(AtomicBool::new(false)),
            reads: Arc::new(Mutex::new(std::collections::BTreeMap::new())),
        }
    }
}
struct PermutedProvider<C: CurveAffine> {
    inner: MemberProvider<C>,
    controls: Controls,
}
struct PermutedWriter<C: CurveAffine> {
    inner: MemberWriter<C>,
    controls: Controls,
    observations: std::cell::Cell<usize>,
}
struct PermutedSnapshot<C: CurveAffine> {
    inner: MemberSnapshot<C>,
    controls: Controls,
}

fn permuted_change(
    layout: StoredPolynomialLayoutV1,
    change: PermutedChange,
) -> StoredPolynomialLayoutV1 {
    let StoredPolynomialRoleV1::LookupPermuted { lookup, side } = layout.role() else {
        panic!("expected tag4 writer")
    };
    StoredPolynomialLayoutV1::new(
        if change == PermutedChange::Context {
            [41; 32]
        } else {
            [23; 32]
        },
        match change {
            PermutedChange::Ordinal => 0,
            PermutedChange::Exhausted => u64::MAX,
            PermutedChange::InsufficientOrdinals => u64::MAX - 3,
            _ => layout.ordinal(),
        },
        if change == PermutedChange::Field {
            match layout.field() {
                StoredPastaFieldV1::Fp => StoredPastaFieldV1::Fq,
                StoredPastaFieldV1::Fq => StoredPastaFieldV1::Fp,
            }
        } else {
            layout.field()
        },
        if change == PermutedChange::Basis {
            match layout.basis() {
                StoredPolynomialBasisV1::Lagrange => StoredPolynomialBasisV1::Coefficient,
                _ => StoredPolynomialBasisV1::Lagrange,
            }
        } else {
            layout.basis()
        },
        if change == PermutedChange::K {
            layout.k() + 1
        } else {
            layout.k()
        },
        match change {
            PermutedChange::Lookup => StoredPolynomialRoleV1::LookupPermuted {
                lookup: lookup + 1,
                side,
            },
            PermutedChange::Side => StoredPolynomialRoleV1::LookupPermuted {
                lookup,
                side: match side {
                    StoredLookupSideV1::Input => StoredLookupSideV1::Table,
                    StoredLookupSideV1::Table => StoredLookupSideV1::Input,
                },
            },
            PermutedChange::CompressedRole => {
                StoredPolynomialRoleV1::LookupCompressed { lookup, side }
            }
            _ => layout.role(),
        },
    )
    .unwrap()
}
impl<C: CurveAffine> StoredPolynomialProviderV1 for PermutedProvider<C> {
    type Writer = PermutedWriter<C>;
    fn create(
        &mut self,
        field: StoredPastaFieldV1,
        basis: StoredPolynomialBasisV1,
        k: u32,
        role: StoredPolynomialRoleV1,
    ) -> Result<Self::Writer, StoredPolynomialErrorV1> {
        let mut inner = self.inner.create(field, basis, k, role)?;
        if matches!(role, StoredPolynomialRoleV1::LookupPermuted { .. }) {
            if let PermutedFault::CreateLayout(change) = *self.controls.metadata.lock().unwrap() {
                inner.inner.layout = permuted_change(inner.inner.layout, change);
            }
        }
        Ok(PermutedWriter {
            inner,
            controls: self.controls.clone(),
            observations: std::cell::Cell::new(0),
        })
    }
}
impl<C: CurveAffine> StoredPolynomialWriterV1 for PermutedWriter<C> {
    type Snapshot = PermutedSnapshot<C>;
    fn layout(&self) -> StoredPolynomialLayoutV1 {
        let count = self.observations.get();
        self.observations.set(count + 1);
        let layout = self.inner.layout();
        let fault = *self.controls.metadata.lock().unwrap();
        if matches!(layout.role(), StoredPolynomialRoleV1::LookupPermuted { .. })
            && ((fault == PermutedFault::WriterSecondObservation && count > 0)
                || (fault == PermutedFault::WriterAfterWrite && self.inner.inner.next > 0))
        {
            different_context(layout)
        } else {
            layout
        }
    }
    fn write_chunk(
        &mut self,
        chunk: u64,
        values: &[[u8; 32]],
    ) -> Result<(), StoredPolynomialErrorV1> {
        self.inner.write_chunk(chunk, values)
    }
    fn seal(self) -> Result<Self::Snapshot, StoredPolynomialErrorV1> {
        Ok(PermutedSnapshot {
            inner: self.inner.seal()?,
            controls: self.controls,
        })
    }
}
impl<C: CurveAffine> StoredPolynomialSnapshotV1 for PermutedSnapshot<C> {
    fn layout(&self) -> StoredPolynomialLayoutV1 {
        self.inner.layout()
    }
    fn with_chunk<V>(
        &mut self,
        expected: StoredPolynomialLayoutV1,
        chunk: u64,
        consume: impl FnOnce(&[[u8; 32]]) -> Result<V, StoredPolynomialErrorV1>,
    ) -> Result<V, StoredPolynomialErrorV1> {
        let occurrence = {
            let mut reads = self.controls.reads.lock().unwrap();
            let count = reads.entry((expected.ordinal(), chunk)).or_default();
            *count += 1;
            *count
        };
        let fault = *self.controls.metadata.lock().unwrap();
        if let PermutedFault::ReadOccurrence {
            ordinal,
            chunk: target,
            occurrence: nth,
            panic,
        } = fault
        {
            if ordinal == expected.ordinal() && target == chunk && nth == occurrence {
                assert!(!panic, "injected permutation reread unwind");
                return Err(StoredPolynomialErrorV1::Storage);
            }
        }
        self.inner.with_chunk(expected, chunk, consume)
    }
    fn with_column<V>(
        &mut self,
        _: StoredPolynomialLayoutV1,
        _: impl FnOnce(&[[u8; 32]]) -> Result<V, StoredPolynomialErrorV1>,
    ) -> Result<V, StoredPolynomialErrorV1> {
        panic!("permuted stage requested backend full-column allocation")
    }
}
struct BoundaryRng<C: CurveAffine> {
    inner: Rng<C>,
    controls: Controls,
}

// Counting the actual field sampler's RngCore calls keeps fault placement independent of
// whether a supported Pasta implementation uses fill_bytes or several next_u64 calls.
struct CountedRng<R> {
    inner: R,
    calls: usize,
}
impl<R: RngCore> RngCore for CountedRng<R> {
    fn next_u32(&mut self) -> u32 {
        self.calls += 1;
        self.inner.next_u32()
    }
    fn next_u64(&mut self) -> u64 {
        self.calls += 1;
        self.inner.next_u64()
    }
    fn fill_bytes(&mut self, output: &mut [u8]) {
        self.calls += 1;
        self.inner.fill_bytes(output);
    }
    fn try_fill_bytes(&mut self, output: &mut [u8]) -> Result<(), RngError> {
        self.calls += 1;
        self.inner.try_fill_bytes(output)
    }
}
impl<C: CurveAffine> BoundaryRng<C> {
    fn check(&self) {
        let fault = *self.controls.boundary.lock().unwrap();
        let count = self.inner.0.log.lock().unwrap().rng_calls;
        assert_ne!(
            fault,
            BoundaryFault::Rng(count),
            "injected lookup RNG unwind"
        );
    }
}
impl<C: CurveAffine> RngCore for BoundaryRng<C> {
    fn next_u32(&mut self) -> u32 {
        self.check();
        self.inner.next_u32()
    }
    fn next_u64(&mut self) -> u64 {
        self.check();
        self.inner.next_u64()
    }
    fn fill_bytes(&mut self, bytes: &mut [u8]) {
        self.check();
        self.inner.fill_bytes(bytes);
    }
    fn try_fill_bytes(&mut self, bytes: &mut [u8]) -> Result<(), RngError> {
        self.check();
        self.inner.try_fill_bytes(bytes)
    }
}
struct BoundaryTranscript<C: CurveAffine>
where
    C::Scalar: FromUniformBytes<64>,
{
    inner: RecordingTranscript<C>,
    controls: Controls,
}
impl<C: CurveAffine> Transcript<C, Challenge255<C>> for BoundaryTranscript<C>
where
    C::Scalar: FromUniformBytes<64>,
{
    fn squeeze_challenge(&mut self) -> Challenge255<C> {
        self.inner.squeeze_challenge()
    }
    fn common_point(&mut self, point: C) -> io::Result<()> {
        self.inner.common_point(point)
    }
    fn common_scalar(&mut self, scalar: C::Scalar) -> io::Result<()> {
        self.inner.common_scalar(scalar)
    }
}
impl<C: CurveAffine> TranscriptWrite<C, Challenge255<C>> for BoundaryTranscript<C>
where
    C::Scalar: FromUniformBytes<64>,
{
    fn write_point(&mut self, point: C) -> io::Result<()> {
        let fault = *self.controls.boundary.lock().unwrap();
        let count = self.inner.shared.log.lock().unwrap().events.len();
        assert_ne!(
            fault,
            BoundaryFault::Transcript(count),
            "injected lookup transcript unwind"
        );
        self.inner.write_point(point)
    }
    fn write_scalar(&mut self, scalar: C::Scalar) -> io::Result<()> {
        self.inner.write_scalar(scalar)
    }
}

macro_rules! fault_member {
    ($params:expr, $pk:expr, $instances:expr, $shared:expr, $controls:expr, $four:literal, $empty:literal) => {
        prepare_single_phase_stored_ipa_prefix_v1::<C, _, _, _, Challenge255<C>, _, false, 0>(
            $params,
            $pk,
            MembershipCircuit::<C, $four, $empty>(EmptyLookupProducer(Producer::new($shared, 0))),
            $instances,
            PermutedProvider {
                inner: MemberProvider {
                    inner: Provider::new($shared),
                    fault: Arc::clone(&$controls.backend),
                    window: Arc::clone(&$controls.window),
                },
                controls: $controls.clone(),
            },
            BoundaryRng {
                inner: Rng(Arc::clone($shared)),
                controls: $controls.clone(),
            },
            BoundaryTranscript {
                inner: RecordingTranscript::new($shared),
                controls: $controls.clone(),
            },
        )
        .unwrap()
        .stage_advice_coefficients()
        .unwrap()
        .compress_lookups(1 << 20)
        .unwrap()
        .sort_lookup_values(1 << 20)
        .unwrap()
        .prepare_lookup_membership(1 << 20)
        .unwrap()
    };
}

fn assert_owned_cleanup_with_survivor<C: CurveAffine>(
    shared: &Arc<Shared<C>>,
    survivor: &mut PermutedSnapshot<C>,
    expected: &[[u8; 32]],
) {
    {
        let log = shared.log.lock().unwrap();
        assert_eq!(log.live_producers, 0);
        assert_eq!(log.producer_drops, 1);
        assert_eq!(
            (log.provider_drops, log.rng_drops, log.transcript_drops),
            (1, 1, 1)
        );
        assert_eq!(log.writer_drops, log.created);
        assert_eq!(
            log.snapshot_drops + 1,
            log.sealed.len(),
            "an original or partial owner survived refusal"
        );
    }
    let layout = survivor.layout();
    let mut actual = Vec::new();
    for chunk in 0..layout.chunk_count() as u64 {
        survivor
            .with_chunk(layout, chunk, |values| {
                actual.extend_from_slice(values);
                Ok(())
            })
            .unwrap();
    }
    assert_eq!(
        actual, expected,
        "unrelated bank lost or modified during consuming cleanup"
    );
}

fn permuted_backend_faults<C>(metadata: bool)
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(9);
    let pk = membership_key::<C, false, false>(&params);
    let usable = 512 - (pk.vk.cs.blinding_factors() + 1);
    let values = fixture_values::<C::Scalar>(usable, false);
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    for case in 0..if metadata { 27 } else { 17 } {
        let shared = Shared::<C>::new();
        let controls = Controls::new();
        let mut member = fault_member!(
            &params,
            pk.clone(),
            &instances,
            &shared,
            controls,
            false,
            false
        );
        // The unrelated bank belongs to this same backend, but never enters the consuming
        // protocol owner. A failure must destroy exactly owned banks and leave this one alive.
        let mut sentinel_writer = member
            .compressed
            .inner
            .provider
            .create(
                C::Scalar::STORED_FIELD,
                StoredPolynomialBasisV1::Lagrange,
                9,
                StoredPolynomialRoleV1::Advice {
                    column: 99,
                    phase: 0,
                },
            )
            .unwrap();
        let sentinel_values = vec![C::Scalar::from(91).to_repr(); 512];
        for chunk in 0..2 {
            sentinel_writer
                .write_chunk(
                    chunk,
                    &sentinel_values[chunk as usize * 256..(chunk as usize + 1) * 256],
                )
                .unwrap();
        }
        let mut survivor = sentinel_writer.seal().unwrap();
        let first_output = member.compressed.inner.provider.inner.inner.ordinal;
        let input = member.lookups[0].input.layout.ordinal();
        let leftover = member.lookups[0].leftover_table.layout.ordinal();
        let later_input = member.lookups[1].input.layout.ordinal();
        let later_leftover = member.lookups[1].leftover_table.layout.ordinal();
        let original = member.compressed.lookups[0].input.layout.ordinal();
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
        let live = sealed.len() - shared.log.lock().unwrap().snapshot_drops;
        controls.reads.lock().unwrap().clear();
        let mut expected_panic = false;
        if metadata {
            let mutation = match case {
                0 => PermutedFault::CreateLayout(PermutedChange::Context),
                1 => PermutedFault::CreateLayout(PermutedChange::Ordinal),
                2 => PermutedFault::CreateLayout(PermutedChange::Exhausted),
                3 => PermutedFault::CreateLayout(PermutedChange::Field),
                4 => PermutedFault::CreateLayout(PermutedChange::Basis),
                5 => PermutedFault::CreateLayout(PermutedChange::K),
                6 => PermutedFault::CreateLayout(PermutedChange::Lookup),
                7 => PermutedFault::CreateLayout(PermutedChange::Side),
                8 => PermutedFault::CreateLayout(PermutedChange::CompressedRole),
                9 => PermutedFault::WriterSecondObservation,
                10 => PermutedFault::WriterAfterWrite,
                26 => PermutedFault::CreateLayout(PermutedChange::InsufficientOrdinals),
                _ => PermutedFault::None,
            };
            *controls.metadata.lock().unwrap() = mutation;
            *controls.backend.lock().unwrap() = match case {
                11 => MemberFault::SnapshotLayout(input),
                12 => MemberFault::SnapshotLayout(later_leftover),
                13 => MemberFault::AfterRead(input, 0, original),
                14 => MemberFault::AfterRead(leftover, 0, later_input),
                15 => MemberFault::AfterRead(later_input, 0, first_output),
                16 => MemberFault::AfterSeal(first_output, original),
                17 => MemberFault::AfterSeal(first_output, later_leftover),
                18 => MemberFault::AfterSeal(first_output + 1, input),
                19 => MemberFault::AfterSeal(first_output + 2, first_output),
                20 => MemberFault::AfterSeal(first_output + 3, first_output + 2),
                21 => MemberFault::AfterSeal(first_output + 7, first_output),
                22 => MemberFault::AfterSeal(first_output + 7, first_output + 7),
                23 => MemberFault::AfterRead(first_output, 1, original),
                24 => MemberFault::AfterRead(first_output + 1, 1, first_output + 2),
                25 => MemberFault::AfterRead(later_leftover, 0, later_leftover),
                _ => MemberFault::None,
            };
        } else {
            *controls.backend.lock().unwrap() = match case {
                0 => MemberFault::Create(first_output + 7),
                1 => MemberFault::Write(first_output, 1),
                2 => MemberFault::Write(first_output + 7, 1),
                3 => MemberFault::Seal(first_output + 7),
                4 => MemberFault::Read(input, 1),
                5 => MemberFault::Read(later_leftover, 0),
                6 => MemberFault::PanicWrite(first_output + 7, 1),
                7 => MemberFault::PanicSeal(first_output + 7),
                8 => MemberFault::PanicRead(later_input, 1),
                9 => MemberFault::Encoding(input, 1),
                10 => MemberFault::Encoding(later_leftover, 0),
                11 => MemberFault::ShortChunk(input, 1),
                12 => MemberFault::ShortChunk(later_leftover, 0),
                13 => MemberFault::Capacity(live),
                _ => MemberFault::None,
            };
            expected_panic = matches!(case, 6..=8 | 16);
            if case >= 14 {
                *controls.metadata.lock().unwrap() = PermutedFault::ReadOccurrence {
                    ordinal: first_output,
                    chunk: 1,
                    occurrence: if case == 14 { 1 } else { 2 },
                    panic: case == 16,
                };
            }
        }
        take_permuted_clears();
        let result = catch_unwind(AssertUnwindSafe(|| {
            member
                .commit_permuted_lookups(
                    permuted_scratch_bytes::<C, PermutedSnapshot<C>>(9, 2).unwrap(),
                )
                .map(|_| ())
        }));
        if expected_panic {
            assert!(result.is_err(), "unreached unwind case {metadata}/{case}");
        } else {
            assert!(
                result.unwrap().is_err(),
                "admitted injected fault {metadata}/{case}"
            );
        }
        assert!(!controls.window.load(Ordering::SeqCst));
        let (_, fields_zero, _, bytes_zero, _, blinds_zero) = take_permuted_clears();
        assert!(
            fields_zero && bytes_zero && blinds_zero,
            "initialized guarded data survived {metadata}/{case}"
        );
        {
            let log = shared.log.lock().unwrap();
            assert_eq!(&log.sealed[..sealed.len()], &sealed);
            assert_eq!(&log.events[..events.len()], &events);
            if metadata && (case <= 9 || matches!(case, 11 | 12 | 26)) {
                assert_eq!(log.reads, reads, "identity admission reached witness I/O");
                assert_eq!(log.writes, writes);
                assert_eq!(log.rng_calls, draws);
                assert_eq!(log.events, events);
            }
            if metadata && (13..=25).contains(&case) {
                assert!(
                    log.snapshot_layout_override.is_some(),
                    "late metadata trigger was not reached for case {case}"
                );
            }
        }
        if !metadata && case >= 14 {
            assert_eq!(
                controls
                    .reads
                    .lock()
                    .unwrap()
                    .get(&(first_output, 1))
                    .copied(),
                Some(if case == 14 { 1 } else { 2 }),
                "conversion/commit reread fault was not reached"
            );
        }
        // Turn off fault injection only after the consumed owner has already dropped; then
        // authenticate and read the one unrelated survivor to distinguish scoped cleanup.
        *controls.backend.lock().unwrap() = MemberFault::None;
        *controls.metadata.lock().unwrap() = PermutedFault::None;
        assert_owned_cleanup_with_survivor(&shared, &mut survivor, &sentinel_values);
        drop(survivor);
        assert_dropped(&shared);
    }
}

#[test]
fn both_pasta_permuted_context_role_basis_ordinal_and_late_receipt_changes_destroy_only_owned_banks()
 {
    permuted_backend_faults::<EqAffine>(true);
    permuted_backend_faults::<EpAffine>(true);
}
#[test]
fn both_pasta_permuted_partial_io_encoding_capacity_and_reread_unwind_destroy_only_owned_banks() {
    permuted_backend_faults::<EqAffine>(false);
    permuted_backend_faults::<EpAffine>(false);
}

fn permuted_boundaries<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(4);
    let pk = membership_key::<C, true, false>(&params);
    let values = fixture_values::<C::Scalar>(4, true);
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    for case in 0..14 {
        let shared = Shared::<C>::new();
        let controls = Controls::new();
        let member = fault_member!(
            &params,
            pk.clone(),
            &instances,
            &shared,
            controls,
            true,
            false
        );
        let (events, draws, sealed) = {
            let log = shared.log.lock().unwrap();
            (log.events.clone(), log.rng_calls, log.sealed.len())
        };
        let tail = 16 - member.usable_rows;
        // The first six offsets cover initial and partially initialized input/table tails,
        // both single-blind samples, and the next lookup after two successful point writes.
        // Probe only the actual scalar sampler's call counts using the same current RNG
        // state. Full permutation/commitment correctness is checked by the ordinary oracle
        // above; this count probe does not implement or substitute that oracle.
        let mut counted = CountedRng {
            inner: shared.rng.lock().unwrap().clone(),
            calls: 0,
        };
        let mut sample_offsets = Vec::new();
        for _ in 0..=2 * tail + 2 {
            sample_offsets.push(counted.calls);
            let _ = C::Scalar::random(&mut counted);
        }
        let rng_offset =
            [0, 1, tail, 2 * tail, 2 * tail + 1, 2 * tail + 2].map(|sample| sample_offsets[sample]);
        if case < 6 {
            *controls.boundary.lock().unwrap() = BoundaryFault::Rng(draws + rng_offset[case]);
        } else if case < 10 {
            shared.log.lock().unwrap().fault = Some(Fault::Transcript(events.len() + case - 6));
        } else {
            *controls.boundary.lock().unwrap() =
                BoundaryFault::Transcript(events.len() + case - 10);
        }
        take_permuted_clears();
        let result = catch_unwind(AssertUnwindSafe(|| {
            member
                .commit_permuted_lookups(
                    permuted_scratch_bytes::<C, PermutedSnapshot<C>>(4, 2).unwrap(),
                )
                .map(|_| ())
        }));
        if (6..10).contains(&case) {
            assert_eq!(result.unwrap(), Err(StoredLookupErrorV1::Transcript));
        } else {
            assert!(
                result.is_err(),
                "RNG/transcript unwind boundary {case} was not reached"
            );
        }
        assert_dropped(&shared);
        let log = shared.log.lock().unwrap();
        assert_eq!(&log.events[..events.len()], &events);
        if case < 6 {
            assert_eq!(log.rng_calls, draws + rng_offset[case]);
            assert_eq!(
                log.events.len(),
                events.len() + if case == 5 { 2 } else { 0 }
            );
        } else {
            let failed_point = if case < 10 { case - 6 } else { case - 10 };
            assert_eq!(log.events.len(), events.len() + failed_point);
            assert_eq!(
                log.sealed.len(),
                sealed + if failed_point < 2 { 4 } else { 8 }
            );
        }
        let (fields, fields_zero, encoded, bytes_zero, blinds, blinds_zero) =
            take_permuted_clears();
        assert!(fields >= 16 && encoded >= 256);
        assert!(fields_zero && bytes_zero && blinds_zero);
        if case >= 6 {
            assert_eq!(blinds, if (case - 6) % 4 < 2 { 2 } else { 4 });
        }
    }
}

#[test]
fn both_pasta_permuted_rng_and_each_transcript_error_or_unwind_destroy_all_owned_state() {
    permuted_boundaries::<EqAffine>();
    permuted_boundaries::<EpAffine>();
}

fn permuted_preflights<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(4);
    let wrong_params = ParamsIPA::<C>::new(5);
    let pk = membership_key::<C, true, false>(&params);
    let values = fixture_values::<C::Scalar>(4, true);
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    for case in 0..12 {
        let shared = Shared::<C>::new();
        let controls = Controls::new();
        let mut member = fault_member!(
            &params,
            pk.clone(),
            &instances,
            &shared,
            controls,
            true,
            false
        );
        let mut budget = permuted_scratch_bytes::<C, PermutedSnapshot<C>>(4, 2).unwrap();
        match case {
            0 => budget = 0,
            1 => budget -= 1,
            2 => {
                member.lookups.pop();
            }
            3 => {
                member.compressed.lookups.pop();
            }
            4 => member.compressed.inner.params = &wrong_params,
            5 => member.usable_rows += 1,
            6 => member.lookups[1].leftover_rows += 1,
            7 => member.lookups[0].distinct_inputs += 1,
            8 => override_receipt(&shared, member.compressed.lookups[0].input.layout.ordinal()),
            9 => {
                let ordinal = member
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
            10 => {
                let column = &mut member.lookups[1].leftover_table;
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
                column.snapshot.inner.inner.layout = layout;
            }
            11 => {
                // Correct context and geometry cannot make a prior lookup's input receipt
                // serve as another lookup's input; swap both immutable receipt and snapshot.
                member.lookups.swap(0, 1);
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
        take_permuted_clears();
        let result = member.commit_permuted_lookups(budget).map(|_| ());
        if case <= 1 {
            assert_eq!(result, Err(StoredLookupErrorV1::ScratchLimit));
        } else {
            assert!(result.is_err(), "public preflight {case} admitted");
        }
        assert_dropped(&shared);
        let log = shared.log.lock().unwrap();
        assert_eq!(log.events, events);
        assert_eq!(log.rng_calls, draws);
        assert_eq!(
            log.reads, reads,
            "public preflight {case} reached witness bytes"
        );
        assert_eq!(log.writes, writes);
        assert_eq!(log.created, created);
        assert_eq!(take_permuted_clears(), (0, true, 0, true, 0, true));
    }
}

#[test]
fn both_pasta_permuted_budget_exact_key_population_receipt_and_overflow_preflights_have_no_io() {
    permuted_preflights::<EqAffine>();
    permuted_preflights::<EpAffine>();
}

fn empty_permuted<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(4);
    let pk = membership_key::<C, true, true>(&params);
    let values = fixture_values::<C::Scalar>(4, true);
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let shared = Shared::<C>::new();
    let controls = Controls::new();
    let member = fault_member!(&params, pk, &instances, &shared, controls, true, true);
    assert_eq!(
        permuted_scratch_bytes::<C, PermutedSnapshot<C>>(4, 0).unwrap(),
        0
    );
    assert_eq!(
        permuted_scratch_bytes::<C, PermutedSnapshot<C>>(4, usize::MAX),
        Err(StoredLookupErrorV1::Context)
    );
    assert_eq!(
        permuted_scratch_bytes::<C, PermutedSnapshot<C>>(u32::MAX, 1),
        Err(StoredLookupErrorV1::Context)
    );
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
    let advice = member
        .compressed
        .inner
        .advice
        .layouts()
        .unwrap()
        .collect::<Vec<_>>();
    let mut expected_rng = shared.rng.lock().unwrap().clone();
    take_permuted_clears();
    let mut permuted = member.commit_permuted_lookups(0).unwrap();
    assert!(permuted.lookups.is_empty());
    assert!(permuted.compressed.lookups.is_empty());
    assert_eq!(
        permuted
            .compressed
            .inner
            .advice
            .layouts()
            .unwrap()
            .collect::<Vec<_>>(),
        advice
    );
    assert_eq!(take_permuted_clears(), (0, true, 0, true, 0, true));
    {
        let log = shared.log.lock().unwrap();
        assert_eq!(log.events, events);
        assert_eq!(log.rng_calls, draws);
        assert_eq!(log.reads, reads);
        assert_eq!(log.writes, writes);
        assert_eq!(log.created, created);
        assert_eq!(log.sealed, sealed);
        assert_eq!(
            (log.provider_drops, log.rng_drops, log.transcript_drops),
            (0, 0, 0)
        );
    }
    let (mut actual, mut expected) = ([0; 64], [0; 64]);
    permuted.compressed.inner.rng.fill_bytes(&mut actual);
    expected_rng.fill_bytes(&mut expected);
    assert_eq!(actual, expected);
    drop(permuted);
    assert_dropped(&shared);
}

#[test]
fn both_pasta_zero_permutations_preserve_original_advice_rng_and_transcript_without_scratch() {
    empty_permuted::<EqAffine>();
    empty_permuted::<EpAffine>();
}

#[test]
fn permuted_tag4_digest_binds_side_lookup_field_and_each_basis_without_advice_aliasing() {
    let mut digests = std::collections::BTreeSet::new();
    for (field_byte, field) in [StoredPastaFieldV1::Fp, StoredPastaFieldV1::Fq]
        .into_iter()
        .enumerate()
    {
        for lookup in [0_u32, 7] {
            for (side_byte, side) in [StoredLookupSideV1::Input, StoredLookupSideV1::Table]
                .into_iter()
                .enumerate()
            {
                for (basis_byte, extension, part, basis) in [
                    (0_u8, 0_u32, 0_u32, StoredPolynomialBasisV1::Lagrange),
                    (1, 0, 0, StoredPolynomialBasisV1::Coefficient),
                    (
                        2,
                        2,
                        0,
                        StoredPolynomialBasisV1::CosetPart {
                            extension_log: 2,
                            part: 0,
                        },
                    ),
                    (
                        2,
                        2,
                        3,
                        StoredPolynomialBasisV1::CosetPart {
                            extension_log: 2,
                            part: 3,
                        },
                    ),
                ] {
                    let role = StoredPolynomialRoleV1::LookupPermuted { lookup, side };
                    let layout =
                        StoredPolynomialLayoutV1::new([23; 32], 19, field, basis, 9, role).unwrap();
                    assert_eq!(layout.role(), role);
                    assert_eq!(
                        layout.advice_coordinates(),
                        Err(StoredPolynomialErrorV1::Context)
                    );
                    // Independent fixed-layout binding oracle pins the new numeric tag, rather
                    // than merely proving a digest is self-consistent with its own encoder.
                    let mut hash = blake2b_simd::Params::new()
                        .hash_length(32)
                        .personal(b"Halo2PolyStoreV1")
                        .to_state();
                    hash.update(b"polynomial.snapshot.v1\0canonical-primefield-repr\0zero-tail\0");
                    hash.update(&[23; 32]);
                    hash.update(&19_u64.to_le_bytes());
                    hash.update(&[field_byte as u8, basis_byte]);
                    hash.update(&extension.to_le_bytes());
                    hash.update(&part.to_le_bytes());
                    hash.update(&9_u32.to_le_bytes());
                    hash.update(&[4]);
                    hash.update(&lookup.to_le_bytes());
                    hash.update(&[side_byte as u8]);
                    hash.update(&512_u64.to_le_bytes());
                    hash.update(&256_u64.to_le_bytes());
                    hash.update(&32_u64.to_le_bytes());
                    assert_eq!(
                        layout.context_digest().as_slice(),
                        hash.finalize().as_bytes()
                    );
                    assert!(
                        digests.insert(layout.context_digest()),
                        "tag4 coordinates aliased"
                    );
                    for old_role in [
                        StoredPolynomialRoleV1::Advice {
                            column: lookup,
                            phase: side_byte as u8,
                        },
                        StoredPolynomialRoleV1::LookupCompressed { lookup, side },
                    ] {
                        let old =
                            StoredPolynomialLayoutV1::new([23; 32], 19, field, basis, 9, old_role)
                                .unwrap();
                        assert_ne!(old.context_digest(), layout.context_digest());
                    }
                    if basis == StoredPolynomialBasisV1::Lagrange {
                        for old_role in [
                            StoredPolynomialRoleV1::LookupLeftoverTable { lookup },
                            StoredPolynomialRoleV1::LookupSorted {
                                lookup,
                                side,
                                run_log: 9,
                            },
                        ] {
                            let old = StoredPolynomialLayoutV1::new(
                                [23; 32], 19, field, basis, 9, old_role,
                            )
                            .unwrap();
                            assert_ne!(old.context_digest(), layout.context_digest());
                        }
                    }
                }
            }
        }
    }
    assert_eq!(digests.len(), 32);
}

#[path = "products_tests.rs"]
mod products;
