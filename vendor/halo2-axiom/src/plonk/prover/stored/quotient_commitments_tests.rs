//! Independent ordinary quotient construction and complete stored commitment owner tests.
//!
//! Tiny plaintext fixtures verify the commitment boundary through ChallengeX, not complete
//! proofs, encrypted artifact authority, process RSS, latency or mobile qualification.

use super::*;
use crate::plonk::prover::stored::quotient_commitments::{StageEvent, take_stage_observations};
use crate::{
    plonk::{ChallengeBeta, ChallengeGamma, ChallengeX, ChallengeY},
    poly::commitment::{Blind, Params as _, ParamsProver as _},
};
use group::Curve;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProtocolKind {
    Next32,
    Next64,
    Fill,
    TryFill,
    Point,
    Squeeze,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProtocolFault {
    Panic,
    Error,
    Drift(u64),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ProtocolEvent {
    kind: ProtocolKind,
    backend_events: usize,
}
#[derive(Default)]
struct ProtocolLog {
    active: bool,
    events: Vec<ProtocolEvent>,
    fault: Option<(usize, ProtocolFault)>,
}
struct ProtocolControls {
    log: Mutex<ProtocolLog>,
    storage: Arc<Controls>,
    force_zero: AtomicBool,
}
impl ProtocolControls {
    fn new(storage: &Arc<Controls>) -> Arc<Self> {
        Arc::new(Self {
            log: Mutex::new(ProtocolLog::default()),
            storage: Arc::clone(storage),
            force_zero: AtomicBool::new(false),
        })
    }
    fn arm(&self, fault: Option<(usize, ProtocolFault)>) {
        let mut log = self.log.lock().unwrap();
        log.active = true;
        log.events.clear();
        log.fault = fault;
    }
    fn before(&self, kind: ProtocolKind) -> Option<ProtocolFault> {
        let mut log = self.log.lock().unwrap();
        if !log.active {
            return None;
        }
        let index = log.events.len();
        log.events.push(ProtocolEvent {
            kind,
            backend_events: self.storage.bank.lock().unwrap().events.len(),
        });
        let action = if log.fault.is_some_and(|(target, _)| target == index) {
            log.fault.take().map(|(_, action)| action)
        } else {
            None
        };
        drop(log);
        if action == Some(ProtocolFault::Panic) {
            panic!("injected original commitment protocol unwind");
        }
        action
    }
    fn after(&self, action: Option<ProtocolFault>) {
        if let Some(ProtocolFault::Drift(ordinal)) = action {
            let expected = *self
                .storage
                .bank
                .lock()
                .unwrap()
                .live
                .get(&ordinal)
                .unwrap();
            self.storage.after(Some(Action::Drift(ordinal)), expected);
        }
    }
}
struct CommitmentRng<C: CurveAffine> {
    inner: QuietRng<C>,
    controls: Arc<ProtocolControls>,
}
impl<C: CurveAffine> RngCore for CommitmentRng<C> {
    fn next_u32(&mut self) -> u32 {
        let action = self.controls.before(ProtocolKind::Next32);
        assert_ne!(action, Some(ProtocolFault::Error));
        let value = self.inner.next_u32();
        self.controls.after(action);
        if self.controls.force_zero.load(Ordering::SeqCst) {
            0
        } else {
            value
        }
    }
    fn next_u64(&mut self) -> u64 {
        let action = self.controls.before(ProtocolKind::Next64);
        assert_ne!(action, Some(ProtocolFault::Error));
        let value = self.inner.next_u64();
        self.controls.after(action);
        if self.controls.force_zero.load(Ordering::SeqCst) {
            0
        } else {
            value
        }
    }
    fn fill_bytes(&mut self, bytes: &mut [u8]) {
        let action = self.controls.before(ProtocolKind::Fill);
        assert_ne!(action, Some(ProtocolFault::Error));
        self.inner.fill_bytes(bytes);
        if self.controls.force_zero.load(Ordering::SeqCst) {
            bytes.fill(0);
        }
        self.controls.after(action);
    }
    fn try_fill_bytes(&mut self, bytes: &mut [u8]) -> Result<(), RngError> {
        let action = self.controls.before(ProtocolKind::TryFill);
        assert_ne!(action, Some(ProtocolFault::Error));
        let result = self.inner.try_fill_bytes(bytes);
        if self.controls.force_zero.load(Ordering::SeqCst) {
            bytes.fill(0);
        }
        self.controls.after(action);
        result
    }
}
struct CommitmentTranscript<C: CurveAffine>
where
    C::Scalar: FromUniformBytes<64>,
{
    inner: QuietTranscript<C>,
    controls: Arc<ProtocolControls>,
}
impl<C: CurveAffine> Transcript<C, Challenge255<C>> for CommitmentTranscript<C>
where
    C::Scalar: FromUniformBytes<64>,
{
    fn squeeze_challenge(&mut self) -> Challenge255<C> {
        let action = self.controls.before(ProtocolKind::Squeeze);
        assert_ne!(action, Some(ProtocolFault::Error));
        let value = self.inner.squeeze_challenge();
        self.controls.after(action);
        value
    }
    fn common_point(&mut self, point: C) -> io::Result<()> {
        assert!(
            !self.controls.log.lock().unwrap().active,
            "commitment boundary common point"
        );
        self.inner.common_point(point)
    }
    fn common_scalar(&mut self, scalar: C::Scalar) -> io::Result<()> {
        assert!(
            !self.controls.log.lock().unwrap().active,
            "commitment boundary common scalar"
        );
        self.inner.common_scalar(scalar)
    }
}
impl<C: CurveAffine> TranscriptWrite<C, Challenge255<C>> for CommitmentTranscript<C>
where
    C::Scalar: FromUniformBytes<64>,
{
    fn write_point(&mut self, point: C) -> io::Result<()> {
        let action = self.controls.before(ProtocolKind::Point);
        if action == Some(ProtocolFault::Error) {
            return Err(io::Error::other(
                "injected original commitment transcript failure",
            ));
        }
        let result = self.inner.write_point(point);
        if result.is_ok() {
            self.controls.after(action);
        }
        result
    }
    fn write_scalar(&mut self, scalar: C::Scalar) -> io::Result<()> {
        assert!(
            !self.controls.log.lock().unwrap().active,
            "commitment boundary scalar evaluation"
        );
        self.inner.write_scalar(scalar)
    }
}
macro_rules! commitment_member {
    ($params:expr,$pk:expr,$instances:expr,$shared:expr,$controls:expr,$protocol:expr) => {
        prepare_single_phase_stored_ipa_prefix_v1::<C, _, _, _, Challenge255<C>, _, Q, M>(
            $params,
            $pk,
            InverseCircuit::<C, DEGREE, MIXED, I>(Producer::new($shared, 3)),
            $instances,
            InverseProvider {
                inner: Provider::new($shared),
                controls: Arc::clone($controls),
            },
            CommitmentRng {
                inner: QuietRng {
                    inner: Rng(Arc::clone($shared)),
                    controls: Arc::clone($controls),
                },
                controls: Arc::clone($protocol),
            },
            CommitmentTranscript {
                inner: QuietTranscript {
                    inner: RecordingTranscript::new($shared),
                    controls: Arc::clone($controls),
                },
                controls: Arc::clone($protocol),
            },
        )
        .unwrap()
        .stage_advice_coefficients()
        .unwrap()
        .compress_lookups(1 << 26)
        .unwrap()
        .sort_lookup_values(1 << 26)
        .unwrap()
        .prepare_lookup_membership(1 << 26)
        .unwrap()
    };
}
macro_rules! commitment_input {
    ($params:expr,$pk:expr,$instances:expr,$shared:expr,$controls:expr,$protocol:expr) => {
        commitment_member!($params, $pk, $instances, $shared, $controls, $protocol)
            .commit_permuted_lookups(1 << 26)
            .unwrap()
            .commit_products(1 << 26)
            .unwrap()
            .commit_vanishing_and_stage_coefficients(1 << 26)
            .unwrap()
            .evaluate_quotient_numerator(1 << 26)
            .unwrap()
            .stage_quotient_coefficients(1 << 26)
            .unwrap()
    };
}
struct Ordinary<C: CurveAffine>
where
    C::Scalar: FromUniformBytes<64>,
{
    coefficients: Vec<Vec<C::Scalar>>,
    blinds: Vec<Blind<C::Scalar>>,
    commitments: Vec<C>,
    x: C::Scalar,
    rng: ChaCha20Rng,
    transcript: RecordingTranscript<C>,
    events_before_construct: Vec<Event<C>>,
}
fn ordinary<C>(
    pk: &ProvingKey<C>,
    params: &ParamsIPA<C>,
    theta: crate::plonk::ChallengeTheta<C>,
    advice: &[crate::poly::Polynomial<C::Scalar, crate::poly::LagrangeCoeff>],
    instances: &[crate::poly::Polynomial<C::Scalar, crate::poly::LagrangeCoeff>],
    challenges: &[C::Scalar],
    mut rng: ChaCha20Rng,
    mut transcript: RecordingTranscript<C>,
) -> Ordinary<C>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    // Execute actual ordinary pair/product/vanishing owners; no detached Committed is made.
    let mut pairs = Vec::new();
    for argument in &pk.vk.cs.lookups {
        pairs.push(
            argument
                .commit_permuted(
                    pk,
                    params,
                    &pk.vk.domain,
                    theta,
                    advice,
                    &pk.fixed_values,
                    instances,
                    challenges,
                    &mut rng,
                    &mut transcript,
                )
                .unwrap(),
        );
    }
    let beta: ChallengeBeta<C> = transcript.squeeze_challenge_scalar();
    let gamma: ChallengeGamma<C> = transcript.squeeze_challenge_scalar();
    let permutations = vec![
        pk.vk
            .cs
            .permutation
            .commit(
                params,
                pk,
                &pk.permutation,
                advice,
                &pk.fixed_values,
                instances,
                beta,
                gamma,
                &mut rng,
                &mut transcript,
            )
            .unwrap(),
    ];
    let lookups = vec![
        pairs
            .into_iter()
            .map(|pair| {
                pair.commit_product(pk, params, beta, gamma, &mut rng, &mut transcript)
                    .unwrap()
            })
            .collect::<Vec<_>>(),
    ];
    let vanishing = crate::plonk::vanishing::Argument::<C>::commit(
        params,
        &pk.vk.domain,
        &mut rng,
        &mut transcript,
    )
    .unwrap();
    let y: ChallengeY<C> = transcript.squeeze_challenge_scalar();
    let advice = advice
        .iter()
        .map(|v| pk.vk.domain.lagrange_to_coeff(v.clone()))
        .collect::<Vec<_>>();
    let instances = instances
        .iter()
        .map(|v| pk.vk.domain.lagrange_to_coeff(v.clone()))
        .collect::<Vec<_>>();
    let numerator = pk.ev.evaluate_h(
        pk,
        &[advice.as_slice()],
        &[instances.as_slice()],
        challenges,
        *y,
        *beta,
        *gamma,
        *theta,
        &lookups,
        &permutations,
        false,
    );
    let dense = pk
        .vk
        .domain
        .extended_to_coeff(pk.vk.domain.divide_by_vanishing_poly(numerator.clone()));
    let q = pk.vk.domain.get_quotient_poly_degree();
    let n = params.n() as usize;
    let coefficients = dense[..q * n]
        .chunks_exact(n)
        .map(<[_]>::to_vec)
        .collect::<Vec<_>>();
    let mut shadow_rng = rng.clone();
    let blinds = (0..q)
        .map(|_| Blind(C::Scalar::random(&mut shadow_rng)))
        .collect::<Vec<_>>();
    let events_before_construct = transcript.shared.log.lock().unwrap().events.clone();
    let constructed = vanishing
        .construct(params, &pk.vk.domain, numerator, &mut rng, &mut transcript)
        .unwrap();
    drop(constructed);
    let events = transcript.shared.log.lock().unwrap().events.clone();
    let commitments = events[events_before_construct.len()..]
        .iter()
        .map(|event| match event {
            Event::WritePoint(point) => *point,
            _ => panic!("ordinary construct emitted unexpected event"),
        })
        .collect::<Vec<_>>();
    assert_eq!(commitments.len(), q);
    for ((coefficients, blind), expected) in coefficients.iter().zip(&blinds).zip(&commitments) {
        assert_eq!(
            params
                .commit(&pk.vk.domain.coeff_from_vec(coefficients.clone()), *blind)
                .to_affine(),
            *expected
        );
    }
    let x: ChallengeX<C> = transcript.squeeze_challenge_scalar();
    let mut a = [0; 64];
    let mut b = [0; 64];
    rng.clone().fill_bytes(&mut a);
    shadow_rng.fill_bytes(&mut b);
    assert_eq!(a, b);
    Ordinary {
        coefficients,
        blinds,
        commitments,
        x: *x,
        rng,
        transcript,
        events_before_construct,
    }
}

fn drain_blinds() -> (usize, bool) {
    let (_, _, _, _, count, zero) =
        crate::plonk::prover::stored::lookup_permuted::take_clear_observations();
    (count, zero)
}
fn drain_column() -> (usize, bool) {
    crate::plonk::prover::stored::quotient_commitments::take_clear_observations()
}
fn commitment_success<
    C,
    const DEGREE: usize,
    const MIXED: bool,
    const I: usize,
    const Q: bool,
    const M: u64,
>(
    k: u32,
    empty: bool,
) where
    C: CurveAffine + crate::SerdeCurveAffine,
    C::Scalar: StoredAssignmentFieldV1
        + WithSmallOrderMulGroup<3>
        + FromUniformBytes<64>
        + crate::SerdePrimeField,
{
    let params = ParamsIPA::<C>::new(k);
    let pk = inverse_key::<C, DEGREE, MIXED, I>(&params);
    let values = values::<C, I>(empty);
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let shared = Shared::<C>::new();
    let storage = Controls::new();
    let protocol = ProtocolControls::new(&storage);
    let member = commitment_member!(&params, pk, &instances, &shared, &storage, &protocol);
    let domain = &member.compressed.inner.pk.vk.domain;
    let n = domain.get_n() as usize;
    let q = domain.get_quotient_poly_degree();
    let advice = member
        .compressed
        .inner
        .advice
        .layouts()
        .unwrap()
        .map(|layout| domain.lagrange_from_vec(stored_values::<C>(&shared, layout)))
        .collect::<Vec<_>>();
    let dense_instances = values
        .iter()
        .map(|values| {
            let mut values = values.clone();
            values.resize(n, C::Scalar::ZERO);
            domain.lagrange_from_vec(values)
        })
        .collect::<Vec<_>>();
    let challenges = member
        .compressed
        .inner
        .advice
        .challenges()
        .unwrap()
        .collect::<Vec<_>>();
    let oracle_shared = Shared::<C>::new();
    oracle_shared.log.lock().unwrap().events = shared.log.lock().unwrap().events.clone();
    let oracle_transcript = RecordingTranscript {
        inner: member.compressed.inner.transcript.inner.inner.inner.clone(),
        shared: Arc::clone(&oracle_shared),
    };
    let mut expected = ordinary(
        &member.compressed.inner.pk,
        &params,
        member.compressed.theta,
        &advice,
        &dense_instances,
        &challenges,
        shared.rng.lock().unwrap().clone(),
        oracle_transcript,
    );
    let mut input = member
        .commit_permuted_lookups(1 << 26)
        .unwrap()
        .commit_products(1 << 26)
        .unwrap()
        .commit_vanishing_and_stage_coefficients(1 << 26)
        .unwrap()
        .evaluate_quotient_numerator(1 << 26)
        .unwrap()
        .stage_quotient_coefficients(1 << 26)
        .unwrap();
    assert_eq!(
        shared.log.lock().unwrap().events,
        expected.events_before_construct
    );
    let mut other = sentinel(&mut input.inner.provider, k);
    let originals = storage.bank.lock().unwrap().live.clone();
    let original_values = originals
        .values()
        .map(|layout| (*layout, stored_values::<C>(&shared, *layout)))
        .collect::<Vec<_>>();
    let pieces = input
        .pieces
        .iter()
        .map(|piece| {
            (
                piece.layout,
                piece.snapshot.inner.values.as_ptr(),
                piece.snapshot.inner.values.capacity(),
            )
        })
        .collect::<Vec<_>>();
    let piece_pointer = input.pieces.as_ptr();
    let piece_capacity = input.pieces.capacity();
    let public = public_values(&input.inner.pk);
    let fixed_pointer = input.inner.pk.fixed_polys.as_ptr();
    let sigma_pointer = input.inner.pk.permutation.polys.as_ptr();
    let ev = format!("{:?}", input.inner.pk.ev);
    let vk = input.inner.pk.vk.to_bytes(crate::SerdeFormat::Processed);
    let advice_layouts = input.inner.advice.layouts().unwrap().collect::<Vec<_>>();
    let challenges = input.inner.advice.challenges().unwrap().collect::<Vec<_>>();
    let retained = input
        .inner
        .permutations
        .iter()
        .chain(
            input
                .inner
                .lookups
                .iter()
                .flat_map(|p| [&p.input, &p.table, &p.product]),
        )
        .chain(std::iter::once(&input.inner.random))
        .map(|p| (p.coefficient.layout, (p.blind.0).0, p.commitment))
        .collect::<Vec<_>>();
    let cursor = input.inner.provider.inner.ordinal;
    let greatest = input.inner.advice.greatest_ordinal().unwrap();
    let original_challenges = (
        *input.inner.theta,
        *input.inner.beta,
        *input.inner.gamma,
        *input.inner.y,
    );
    let calls = shared.log.lock().unwrap().rng_calls;
    storage.arm(None);
    protocol.arm(None);
    drain_blinds();
    drain_column();
    take_stage_observations();
    crate::plonk::prover::stored::quotient_commitments::take_reuse_observations();
    let mut actual = input.commit_quotient(1 << 26).unwrap();
    assert_eq!(
        actual.blinds.iter().map(|b| (b.0).0).collect::<Vec<_>>(),
        expected.blinds.iter().map(|b| b.0).collect::<Vec<_>>()
    );
    assert_eq!(actual.commitments, expected.commitments);
    assert_eq!(*actual.x, expected.x);
    assert_eq!(actual.inner.pieces.as_ptr(), piece_pointer);
    assert_eq!(actual.inner.pieces.capacity(), piece_capacity);
    assert_eq!(
        actual
            .inner
            .pieces
            .iter()
            .map(|piece| (
                piece.layout,
                piece.snapshot.inner.values.as_ptr(),
                piece.snapshot.inner.values.capacity()
            ))
            .collect::<Vec<_>>(),
        pieces
    );
    assert_eq!(actual.inner.inner.provider.inner.ordinal, cursor);
    assert_eq!(
        actual.inner.inner.advice.greatest_ordinal().unwrap(),
        greatest
    );
    assert!(std::ptr::eq(actual.inner.inner.params, &params));
    assert_eq!(actual.inner.inner.instances.as_ptr(), instances.as_ptr());
    assert_eq!(public_values(&actual.inner.inner.pk), public);
    assert_eq!(actual.inner.inner.pk.fixed_polys.as_ptr(), fixed_pointer);
    assert_eq!(
        actual.inner.inner.pk.permutation.polys.as_ptr(),
        sigma_pointer
    );
    assert_eq!(format!("{:?}", actual.inner.inner.pk.ev), ev);
    assert_eq!(
        actual
            .inner
            .inner
            .pk
            .vk
            .to_bytes(crate::SerdeFormat::Processed),
        vk
    );
    assert_eq!(
        actual
            .inner
            .inner
            .advice
            .layouts()
            .unwrap()
            .collect::<Vec<_>>(),
        advice_layouts
    );
    assert_eq!(
        actual
            .inner
            .inner
            .advice
            .challenges()
            .unwrap()
            .collect::<Vec<_>>(),
        challenges
    );
    assert_eq!(
        (
            *actual.inner.inner.theta,
            *actual.inner.inner.beta,
            *actual.inner.inner.gamma,
            *actual.inner.inner.y
        ),
        original_challenges
    );
    assert_eq!(
        actual
            .inner
            .inner
            .permutations
            .iter()
            .chain(
                actual
                    .inner
                    .inner
                    .lookups
                    .iter()
                    .flat_map(|p| [&p.input, &p.table, &p.product])
            )
            .chain(std::iter::once(&actual.inner.inner.random))
            .map(|p| (p.coefficient.layout, (p.blind.0).0, p.commitment))
            .collect::<Vec<_>>(),
        retained
    );
    assert_eq!(storage.bank.lock().unwrap().live, originals);
    for (layout, values) in original_values {
        assert_eq!(stored_values::<C>(&shared, layout), values);
    }
    let reads = storage.bank.lock().unwrap().events.clone();
    assert_eq!(reads.len(), q * n.div_ceil(256));
    for (index, event) in reads.iter().enumerate() {
        assert_eq!(event.kind, IoKind::Read);
        assert_eq!(event.layout, pieces[index / n.div_ceil(256)].0);
        assert_eq!(event.chunk, (index % n.div_ceil(256)) as u64);
    }
    let events = protocol.log.lock().unwrap().events.clone();
    let first_point = events
        .iter()
        .position(|event| event.kind == ProtocolKind::Point)
        .unwrap();
    assert!(first_point > 0);
    assert!(events[..first_point].iter().all(|event| !matches!(
        event.kind,
        ProtocolKind::Point | ProtocolKind::Squeeze
    ) && event.backend_events == 0));
    assert_eq!(
        events[first_point..]
            .iter()
            .map(|event| event.kind)
            .collect::<Vec<_>>(),
        std::iter::repeat_n(ProtocolKind::Point, q)
            .chain(std::iter::once(ProtocolKind::Squeeze))
            .collect::<Vec<_>>()
    );
    assert!(
        events[first_point..]
            .iter()
            .all(|event| event.backend_events == reads.len())
    );
    assert_eq!(shared.log.lock().unwrap().rng_calls - calls, first_point);
    assert_eq!(
        shared.log.lock().unwrap().events,
        oracle_shared.log.lock().unwrap().events
    );
    assert_eq!(
        actual
            .inner
            .inner
            .transcript
            .inner
            .inner
            .inner
            .clone()
            .finalize(),
        expected.transcript.inner.clone().finalize()
    );
    assert!(Arc::ptr_eq(&actual.inner.inner.rng.inner.inner.0, &shared));
    assert!(Arc::ptr_eq(
        &actual.inner.inner.transcript.inner.inner.shared,
        &shared
    ));
    let (allocations, capacity, first, last, commits) =
        crate::plonk::prover::stored::quotient_commitments::take_reuse_observations();
    assert_eq!(allocations, 1);
    assert!(capacity >= n);
    assert_ne!(first, 0);
    assert_eq!(first, last);
    assert_eq!(commits, q);
    let mut stages = Vec::new();
    for piece in 0..q {
        stages.extend([StageEvent::SampleStart(piece), StageEvent::SampleEnd(piece)]);
    }
    for piece in 0..q {
        for chunk in 0..n.div_ceil(256) {
            stages.extend([
                StageEvent::ReadStart(piece, chunk),
                StageEvent::ReadEnd(piece, chunk),
            ]);
        }
        stages.extend([StageEvent::CommitStart(piece), StageEvent::CommitEnd(piece)]);
    }
    stages.extend([StageEvent::NormalizeStart, StageEvent::NormalizeEnd]);
    for piece in 0..q {
        stages.extend([StageEvent::WriteStart(piece), StageEvent::WriteEnd(piece)]);
    }
    stages.extend([StageEvent::SqueezeStart, StageEvent::SqueezeEnd]);
    assert_eq!(take_stage_observations(), stages);
    let (cleared, zero) = drain_column();
    assert!(zero && cleared == n * (q + 1));
    assert_eq!(drain_blinds(), (0, true));
    for (piece, expected) in actual.inner.pieces.iter_mut().zip(&expected.coefficients) {
        assert_eq!(read_values(&mut piece.snapshot, piece.layout), *expected);
    }
    protocol.log.lock().unwrap().active = false;
    let mut next = [0; 64];
    let mut next_expected = [0; 64];
    actual.inner.inner.rng.fill_bytes(&mut next);
    expected.rng.fill_bytes(&mut next_expected);
    assert_eq!(next, next_expected);
    assert_eq!(
        actual
            .inner
            .inner
            .transcript
            .squeeze_challenge()
            .get_scalar(),
        expected.transcript.squeeze_challenge().get_scalar()
    );
    drop(actual);
    assert_eq!(storage.bank.lock().unwrap().live.len(), 1);
    assert_eq!(drain_blinds(), (retained.len() + q, true));
    check_sentinel(&mut other);
    drop(other);
    assert_dropped(&shared);
    assert!(storage.bank.lock().unwrap().live.is_empty());
}
fn success_matrix<C>()
where
    C: CurveAffine + crate::SerdeCurveAffine,
    C::Scalar: StoredAssignmentFieldV1
        + WithSmallOrderMulGroup<3>
        + FromUniformBytes<64>
        + crate::SerdePrimeField,
{
    for k in [4, 8, 9] {
        commitment_success::<C, 3, false, 0, false, 0>(k, true);
        commitment_success::<C, 4, false, 0, false, 0>(k, true);
        commitment_success::<C, 6, true, 4, false, 0>(k, false);
        commitment_success::<C, 6, true, 4, true, 0>(k, true);
        commitment_success::<C, 6, true, 4, true, 6>(k, false);
    }
}
#[test]
fn both_pasta_quotient_commitments_match_actual_ordinary_construct_and_challenge_x_preserving_original_owner_and_one_column()
 {
    success_matrix::<EqAffine>();
    success_matrix::<EpAffine>();
}

fn owned_victims(initial: &BTreeMap<u64, StoredPolynomialLayoutV1>) -> Vec<u64> {
    initial
        .iter()
        .filter(|(_, layout)| layout.role() != StoredPolynomialRoleV1::Instance { column: 777 })
        .map(|(ordinal, _)| *ordinal)
        .collect()
}
fn storage_failures<C>(k: u32, late_only: bool)
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    const DEGREE: usize = 6;
    const MIXED: bool = true;
    const I: usize = 4;
    const Q: bool = false;
    const M: u64 = 0;
    let params = ParamsIPA::<C>::new(k);
    let pk = inverse_key::<C, DEGREE, MIXED, I>(&params);
    let values = values::<C, I>(false);
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let shared = Shared::<C>::new();
    let storage = Controls::new();
    let protocol = ProtocolControls::new(&storage);
    let mut input = commitment_input!(
        &params,
        pk.clone(),
        &instances,
        &shared,
        &storage,
        &protocol
    );
    let other = sentinel(&mut input.inner.provider, k);
    let initial = storage.bank.lock().unwrap().live.clone();
    let victims = owned_victims(&initial);
    assert_eq!(victims.len(), 22);
    storage.arm(None);
    protocol.arm(None);
    let baseline = input.commit_quotient(1 << 26).unwrap();
    let events = storage.bank.lock().unwrap().events.clone();
    assert_eq!(events.len(), 5 * (1usize << k).div_ceil(256));
    assert!(events.iter().all(|event| event.kind == IoKind::Read));
    drop(baseline);
    drop(other);
    assert_dropped(&shared);
    let mut cases = Vec::new();
    for (target, event) in events.iter().enumerate() {
        if late_only && event.chunk != 1 {
            continue;
        }
        for action in [
            Action::Error,
            Action::Panic,
            Action::Short,
            Action::Long,
            Action::Encoding,
        ] {
            cases.push((target, action));
        }
        for change in [
            Change::Field,
            Change::K,
            Change::Context,
            Change::Role,
            Change::Part,
            Change::Extension,
            Change::Ordinal,
            Change::Exhausted,
        ] {
            cases.push((target, Action::Change(change)));
        }
        for victim in &victims {
            cases.push((target, Action::Drift(*victim)));
        }
    }
    assert_eq!(cases.len(), 175);
    for (target, action) in cases {
        let shared = Shared::<C>::new();
        let storage = Controls::new();
        let protocol = ProtocolControls::new(&storage);
        let mut input = commitment_input!(
            &params,
            pk.clone(),
            &instances,
            &shared,
            &storage,
            &protocol
        );
        let mut other = sentinel(&mut input.inner.provider, k);
        let creates = shared.log.lock().unwrap().created;
        let original_blinds = input.inner.permutations.len() + 3 * input.inner.lookups.len() + 1;
        storage.arm(Some((target, action)));
        protocol.arm(None);
        drain_column();
        take_stage_observations();
        drain_blinds();
        let result = catch_unwind(AssertUnwindSafe(|| input.commit_quotient(1 << 26)));
        assert!(
            matches!(result, Err(_) | Ok(Err(_))),
            "commitment accepted read {target} {action:?}"
        );
        assert!(storage.bank.lock().unwrap().fault.is_none());
        let observed = storage.bank.lock().unwrap().events.clone();
        assert_eq!(observed[target], events[target]);
        assert!(
            observed[target + 1..]
                .iter()
                .all(|event| event.kind == IoKind::DropSnapshot)
        );
        assert_eq!(storage.bank.lock().unwrap().live.len(), 1);
        assert_eq!(shared.log.lock().unwrap().created, creates);
        assert!(!storage.window.load(Ordering::SeqCst));
        assert!(
            protocol
                .log
                .lock()
                .unwrap()
                .events
                .iter()
                .all(|event| !matches!(event.kind, ProtocolKind::Point | ProtocolKind::Squeeze))
        );
        assert_eq!(
            take_stage_observations().last(),
            Some(&StageEvent::ReadStart(
                target / (1usize << k).div_ceil(256),
                target % (1usize << k).div_ceil(256)
            ))
        );
        let (cleared, zero) = drain_column();
        assert!(zero && cleared >= 1usize << k);
        assert_eq!(drain_blinds(), (original_blinds + 5, true));
        check_sentinel(&mut other);
        drop(result);
        drop(other);
        assert_dropped(&shared);
        assert!(storage.bank.lock().unwrap().live.is_empty());
    }
}
#[test]
fn both_pasta_quotient_commitment_each_piece_and_late_chunk_storage_corruption_consumes_all_owned_receipts_and_blinds()
 {
    storage_failures::<EqAffine>(4, false);
    storage_failures::<EpAffine>(4, false);
    storage_failures::<EqAffine>(9, true);
    storage_failures::<EpAffine>(9, true);
}

fn protocol_failures<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    const DEGREE: usize = 6;
    const MIXED: bool = true;
    const I: usize = 4;
    const Q: bool = true;
    const M: u64 = 6;
    let params = ParamsIPA::<C>::new(4);
    let pk = inverse_key::<C, DEGREE, MIXED, I>(&params);
    let values = values::<C, I>(false);
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let shared = Shared::<C>::new();
    let storage = Controls::new();
    let protocol = ProtocolControls::new(&storage);
    let mut input = commitment_input!(
        &params,
        pk.clone(),
        &instances,
        &shared,
        &storage,
        &protocol
    );
    let other = sentinel(&mut input.inner.provider, 4);
    let initial = storage.bank.lock().unwrap().live.clone();
    let all = owned_victims(&initial);
    assert_eq!(all.len(), 22);
    let random = input.inner.random.coefficient.layout.ordinal();
    let rng_victims = [
        all[0],
        random,
        input.pieces.first().unwrap().layout.ordinal(),
        input.pieces.last().unwrap().layout.ordinal(),
    ];
    storage.arm(None);
    protocol.arm(None);
    let baseline = input.commit_quotient(1 << 26).unwrap();
    let events = protocol.log.lock().unwrap().events.clone();
    drop(baseline);
    drop(other);
    assert_dropped(&shared);
    let mut cases = Vec::new();
    for (target, event) in events.iter().enumerate() {
        cases.push((target, ProtocolFault::Panic));
        if event.kind == ProtocolKind::Point {
            cases.push((target, ProtocolFault::Error));
        }
        for victim in if matches!(event.kind, ProtocolKind::Point | ProtocolKind::Squeeze) {
            all.as_slice()
        } else {
            rng_victims.as_slice()
        } {
            cases.push((target, ProtocolFault::Drift(*victim)));
        }
    }
    assert_eq!(
        events
            .iter()
            .filter(|event| event.kind == ProtocolKind::Point)
            .count(),
        5
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| event.kind == ProtocolKind::Squeeze)
            .count(),
        1
    );
    assert_eq!(events.last().unwrap().kind, ProtocolKind::Squeeze);
    let rng_calls = events
        .iter()
        .filter(|event| !matches!(event.kind, ProtocolKind::Point | ProtocolKind::Squeeze))
        .count();
    assert!(rng_calls > 0);
    assert_eq!(cases.len(), 5 * rng_calls + 143);
    for (target, action) in cases {
        let shared = Shared::<C>::new();
        let storage = Controls::new();
        let protocol = ProtocolControls::new(&storage);
        let mut input = commitment_input!(
            &params,
            pk.clone(),
            &instances,
            &shared,
            &storage,
            &protocol
        );
        let mut other = sentinel(&mut input.inner.provider, 4);
        let creates = shared.log.lock().unwrap().created;
        let original_blinds = input.inner.permutations.len() + 3 * input.inner.lookups.len() + 1;
        storage.arm(None);
        protocol.arm(Some((target, action)));
        drain_column();
        take_stage_observations();
        drain_blinds();
        let result = catch_unwind(AssertUnwindSafe(|| input.commit_quotient(1 << 26)));
        assert!(
            matches!(result, Err(_) | Ok(Err(_))),
            "accepted protocol {target} {:?} {action:?}",
            events[target]
        );
        assert!(protocol.log.lock().unwrap().fault.is_none());
        let seen = protocol.log.lock().unwrap().events.clone();
        assert_eq!(seen[target], events[target]);
        if matches!(
            events[target].kind,
            ProtocolKind::Point | ProtocolKind::Squeeze
        ) || action == ProtocolFault::Panic
        {
            assert_eq!(seen.len(), target + 1);
        } else {
            assert!(
                seen.iter().all(|event| !matches!(
                    event.kind,
                    ProtocolKind::Point | ProtocolKind::Squeeze
                ))
            );
            assert!(
                storage
                    .bank
                    .lock()
                    .unwrap()
                    .events
                    .iter()
                    .all(|event| event.kind == IoKind::DropSnapshot)
            );
        }
        // The validator brackets a complete Field::random, so an injected drift may be
        // followed by remaining RngCore calls internal to that same field sample.
        assert_eq!(storage.bank.lock().unwrap().live.len(), 1);
        assert_eq!(shared.log.lock().unwrap().created, creates);
        assert!(!storage.window.load(Ordering::SeqCst));
        let stages = take_stage_observations();
        match events[target].kind {
            ProtocolKind::Point => {
                assert!(matches!(stages.last(), Some(StageEvent::WriteStart(_))))
            }
            ProtocolKind::Squeeze => assert_eq!(stages.last(), Some(&StageEvent::SqueezeStart)),
            _ => assert!(matches!(stages.last(), Some(StageEvent::SampleStart(_)))),
        }
        let (cleared, zero) = drain_column();
        assert!(zero && cleared >= 16);
        assert_eq!(drain_blinds(), (original_blinds + 5, true));
        check_sentinel(&mut other);
        drop(result);
        drop(other);
        assert_dropped(&shared);
    }
}
#[test]
fn both_pasta_quotient_commitment_rng_and_every_point_or_challenge_callback_error_unwind_and_owner_drift_fail_closed()
 {
    protocol_failures::<EqAffine>();
    protocol_failures::<EpAffine>();
}

fn preflight<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    const DEGREE: usize = 6;
    const MIXED: bool = true;
    const I: usize = 4;
    const Q: bool = true;
    const M: u64 = 6;
    let params = ParamsIPA::<C>::new(4);
    let alternate = ParamsIPA::<C>::new(4);
    let wrong_k = ParamsIPA::<C>::new(5);
    let pk = inverse_key::<C, DEGREE, MIXED, I>(&params);
    let values = values::<C, I>(false);
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let too_long = vec![C::Scalar::ZERO; 17];
    let bad_instances = vec![too_long.as_slice(); I];
    for case in 0..45 {
        let shared = Shared::<C>::new();
        let storage = Controls::new();
        let protocol = ProtocolControls::new(&storage);
        let mut input = commitment_input!(
            &params,
            pk.clone(),
            &instances,
            &shared,
            &storage,
            &protocol
        );
        let mut other = sentinel(&mut input.inner.provider, 4);
        let minimum = crate::plonk::prover::stored::quotient_commitments::scratch_bytes::<
            C,
            InverseProvider<C>,
        >(&input.inner.pk)
        .unwrap();
        assert!(minimum > 16 * 32);
        let mut budget = 1 << 26;
        match case {
            0 => budget = 0,
            1 => budget = minimum - 1,
            2 => input.inner.usable_rows += 1,
            3 => input.inner.params = &alternate,
            4 => input.inner.params = &wrong_k,
            5 => input.inner.pk.vk.cs_degree = 2,
            6 => {
                input.inner.pk.fixed_polys.pop().unwrap();
            }
            7 => {
                input.inner.pk.permutation.polys.pop().unwrap();
            }
            8 => {
                input.inner.pk.fixed_polys[0].values.pop();
            }
            9 => {
                input.inner.pk.permutation.polys[0].values.pop();
            }
            10 => {
                input.inner.pk.l0.values.pop();
            }
            11 => {
                input.inner.pk.l_last.values.pop();
            }
            12 => {
                input.inner.pk.l_active_row.values.pop();
            }
            13 => input.inner.pk.fixed_values.push(
                input
                    .inner
                    .pk
                    .vk
                    .domain
                    .lagrange_from_vec(vec![C::Scalar::ZERO; 16]),
            ),
            14 => input.inner.pk.permutation.permutations.push(
                input
                    .inner
                    .pk
                    .vk
                    .domain
                    .lagrange_from_vec(vec![C::Scalar::ZERO; 16]),
            ),
            15 => {
                input.inner.instance_coefficients.pop().unwrap();
            }
            16 => input.inner.instances = &[],
            17 => input.inner.instances = &bad_instances,
            18 => {
                input.inner.permutations.pop().unwrap();
            }
            19 => {
                input.inner.lookups.pop().unwrap();
            }
            20 => {
                input.inner.pk.ev.lookups.pop().unwrap();
            }
            21 => {
                input.inner.pk.vk.cs.advice_column_phase.pop().unwrap();
            }
            22 => {
                input.inner.pk.vk.cs.challenge_phase.pop().unwrap();
            }
            23 => input.inner.pk.vk.cs.num_advice_columns += 1,
            24 => input.inner.pk.vk.cs.num_challenges += 1,
            25 => {
                input.inner.pk.vk.cs.permutation.columns[0] =
                    Column::<Instance>::new(99, Instance).into()
            }
            26 => {
                input.inner.pk.vk.cs.permutation.columns[1] =
                    input.inner.pk.vk.cs.permutation.columns[0]
            }
            27 => {
                input.inner.instance_coefficients[0].layout =
                    input.inner.instance_coefficients[1].layout
            }
            28 => {
                input.inner.random.coefficient.layout = input.inner.instance_coefficients[0].layout
            }
            29 => storage.after(
                Some(Action::Drift(
                    input
                        .inner
                        .advice
                        .layouts()
                        .unwrap()
                        .last()
                        .unwrap()
                        .ordinal(),
                )),
                input.pieces[0].layout,
            ),
            30 => input.inner.pk.vk.cs.num_instance_columns = usize::MAX,
            31 => storage.after(
                Some(Action::Drift(
                    input.inner.random.coefficient.layout.ordinal(),
                )),
                input.pieces[0].layout,
            ),
            32 => {
                input.inner.pk.vk.cs.lookups.pop().unwrap();
            }
            33 => input.inner.pk.vk.domain = EvaluationDomain::new(4, 3),
            34 => {
                input.pieces.pop().unwrap();
            }
            35 => input.pieces.swap(0, 1),
            36 => input.pieces[0].layout = input.pieces[1].layout,
            37 => input.pieces[0].layout = changed(input.pieces[0].layout, Change::Part),
            38 => input.pieces[0].layout = changed(input.pieces[0].layout, Change::Field),
            39 => input.pieces[0].layout = changed(input.pieces[0].layout, Change::K),
            40 => input.pieces[0].layout = changed(input.pieces[0].layout, Change::Context),
            41 => input.pieces[0].layout = changed(input.pieces[0].layout, Change::Ordinal),
            42 => storage.after(
                Some(Action::Drift(input.pieces.last().unwrap().layout.ordinal())),
                input.pieces[0].layout,
            ),
            43 => {
                let original = input.pieces[0].layout;
                let mut writer = input
                    .inner
                    .provider
                    .create(
                        original.field(),
                        original.basis(),
                        original.k(),
                        original.role(),
                    )
                    .unwrap();
                let layout = writer.layout();
                for chunk in 0..layout.chunk_count() as u64 {
                    writer
                        .write_chunk(
                            chunk,
                            &vec![
                                C::Scalar::ZERO.to_repr();
                                layout.chunk_scalar_count(chunk).unwrap()
                            ],
                        )
                        .unwrap();
                }
                input.pieces.push(
                    crate::plonk::prover::stored::lookup_permuted::PermutedPolynomialV1 {
                        layout,
                        snapshot: writer.seal().unwrap(),
                    },
                );
            }
            44 => {
                let piece = input.pieces.last_mut().unwrap();
                let old = piece.layout;
                let new = StoredPolynomialLayoutV1::new(
                    [23; 32],
                    old.ordinal() + 1,
                    old.field(),
                    old.basis(),
                    old.k(),
                    old.role(),
                )
                .unwrap();
                piece.layout = new;
                piece.snapshot.inner.layout = new;
            }
            _ => unreachable!(),
        }
        let calls = shared.log.lock().unwrap().rng_calls;
        let events = shared.log.lock().unwrap().events.clone();
        let creates = shared.log.lock().unwrap().created;
        storage.arm(None);
        protocol.arm(None);
        drain_column();
        take_stage_observations();
        drain_blinds();
        let result = input.commit_quotient(budget);
        assert!(result.is_err(), "accepted commitment preflight {case}");
        assert!(protocol.log.lock().unwrap().events.is_empty());
        assert!(
            storage
                .bank
                .lock()
                .unwrap()
                .events
                .iter()
                .all(|event| event.kind == IoKind::DropSnapshot)
        );
        assert_eq!(shared.log.lock().unwrap().rng_calls, calls);
        assert_eq!(shared.log.lock().unwrap().events, events);
        assert_eq!(shared.log.lock().unwrap().created, creates);
        assert_eq!(storage.bank.lock().unwrap().live.len(), 1);
        assert!(drain_column().1);
        assert!(drain_blinds().1);
        check_sentinel(&mut other);
        drop(result);
        drop(other);
        assert_dropped(&shared);
    }
}
#[test]
fn both_pasta_quotient_commitment_budget_geometry_original_key_and_piece_inventory_refusals_precede_rng_storage_and_transcript()
 {
    preflight::<EqAffine>();
    preflight::<EpAffine>();
}

fn cursor_or_capacity<C, const I: usize>(exhausted_cursor: bool)
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    const DEGREE: usize = 6;
    const MIXED: bool = false;
    const Q: bool = false;
    const M: u64 = 0;
    let params = ParamsIPA::<C>::new(4);
    let pk = inverse_key::<C, DEGREE, MIXED, I>(&params);
    let values = values::<C, I>(false);
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let shared = Shared::<C>::new();
    let storage = Controls::new();
    let protocol = ProtocolControls::new(&storage);
    let mut numerator = commitment_member!(&params, pk, &instances, &shared, &storage, &protocol)
        .commit_permuted_lookups(1 << 26)
        .unwrap()
        .commit_products(1 << 26)
        .unwrap()
        .commit_vanishing_and_stage_coefficients(1 << 26)
        .unwrap()
        .evaluate_quotient_numerator(1 << 26)
        .unwrap();
    let mut others = Vec::new();
    if exhausted_cursor {
        others.push(sentinel(&mut numerator.inner.provider, 4));
        numerator.inner.provider.inner.ordinal = u64::MAX - 13;
    }
    let mut input = numerator.stage_quotient_coefficients(1 << 26).unwrap();
    if !exhausted_cursor {
        for _ in 0..8 {
            others.push(sentinel(&mut input.inner.provider, 4));
        }
        assert_eq!(storage.bank.lock().unwrap().live.len(), 512);
    } else {
        assert_eq!(input.inner.provider.inner.ordinal, u64::MAX);
        assert_eq!(
            input.inner.advice.greatest_ordinal().unwrap(),
            Some(u64::MAX - 1)
        );
    }
    let cursor = input.inner.provider.inner.ordinal;
    let layouts = storage.bank.lock().unwrap().live.clone();
    let pieces = input
        .pieces
        .iter()
        .map(|p| stored_values::<C>(&shared, p.layout))
        .collect::<Vec<_>>();
    let mut rng = shared.rng.lock().unwrap().clone();
    let blinds = (0..5)
        .map(|_| Blind(C::Scalar::random(&mut rng)))
        .collect::<Vec<_>>();
    let expected = pieces
        .iter()
        .zip(&blinds)
        .map(|(p, b)| {
            params
                .commit(&input.inner.pk.vk.domain.coeff_from_vec(p.clone()), *b)
                .to_affine()
        })
        .collect::<Vec<_>>();
    storage.arm(None);
    protocol.arm(None);
    let actual = input.commit_quotient(1 << 26).unwrap();
    assert_eq!(actual.commitments, expected);
    assert_eq!(actual.inner.inner.provider.inner.ordinal, cursor);
    assert_eq!(storage.bank.lock().unwrap().live, layouts);
    assert!(
        storage
            .bank
            .lock()
            .unwrap()
            .events
            .iter()
            .all(|event| event.kind == IoKind::Read)
    );
    drop(actual);
    assert_eq!(storage.bank.lock().unwrap().live.len(), others.len());
    for snapshot in &mut others {
        check_sentinel(snapshot);
    }
    drop(others);
    assert_dropped(&shared);
    assert!(storage.bank.lock().unwrap().live.is_empty());
}
#[test]
fn both_pasta_quotient_commitments_need_no_new_handle_or_ordinal_at_512_receipts_and_original_max_minus_one_cursor()
 {
    cursor_or_capacity::<EqAffine, 498>(false);
    cursor_or_capacity::<EpAffine, 498>(false);
    cursor_or_capacity::<EqAffine, 0>(true);
    cursor_or_capacity::<EpAffine, 0>(true);
}

fn zero_and_identity<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    const DEGREE: usize = 6;
    const MIXED: bool = false;
    const I: usize = 0;
    const Q: bool = false;
    const M: u64 = 0;
    let params = ParamsIPA::<C>::new(4);
    let pk = inverse_key::<C, DEGREE, MIXED, I>(&params);
    let instances: Vec<&[C::Scalar]> = Vec::new();
    for identity in [false, true] {
        let shared = Shared::<C>::new();
        let storage = Controls::new();
        let protocol = ProtocolControls::new(&storage);
        let mut input = commitment_input!(
            &params,
            pk.clone(),
            &instances,
            &shared,
            &storage,
            &protocol
        );
        let mut other = sentinel(&mut input.inner.provider, 4);
        // This arithmetic-only fixture deliberately supplies different canonical coefficient
        // contents through the plaintext backend after the genuine prefix. It grants no
        // authenticated-storage integrity or valid-numerator/proof claim. Identity additionally
        // arms the same original-lifetime RNG wrapper to return zeros after delegating entropy.
        for (piece, polynomial) in input.pieces.iter_mut().enumerate() {
            if identity || piece % 2 == 0 {
                polynomial
                    .snapshot
                    .inner
                    .values
                    .fill(C::Scalar::ZERO.to_repr());
            }
        }
        let mut rng = shared.rng.lock().unwrap().clone();
        let blinds = (0..5)
            .map(|_| {
                let value = C::Scalar::random(&mut rng);
                Blind(if identity { C::Scalar::ZERO } else { value })
            })
            .collect::<Vec<_>>();
        let expected = input
            .pieces
            .iter()
            .zip(&blinds)
            .map(|(p, b)| {
                let values = p
                    .snapshot
                    .inner
                    .values
                    .iter()
                    .map(|v| Option::<C::Scalar>::from(C::Scalar::from_repr(*v)).unwrap())
                    .collect();
                params
                    .commit(&input.inner.pk.vk.domain.coeff_from_vec(values), *b)
                    .to_affine()
            })
            .collect::<Vec<_>>();
        if identity {
            assert!(expected.iter().all(|point| *point == C::identity()));
        } else {
            assert!(expected.iter().all(|point| *point != C::identity()));
        }
        let oracle_shared = Shared::<C>::new();
        oracle_shared.log.lock().unwrap().events = shared.log.lock().unwrap().events.clone();
        let mut oracle = RecordingTranscript {
            inner: input.inner.transcript.inner.inner.inner.clone(),
            shared: Arc::clone(&oracle_shared),
        };
        let mut write_error = false;
        for point in &expected {
            if oracle.write_point(*point).is_err() {
                write_error = true;
                break;
            }
        }
        let x = if write_error {
            None
        } else {
            Some(oracle.squeeze_challenge().get_scalar())
        };
        assert_eq!(write_error, identity);
        storage.arm(None);
        protocol.arm(None);
        protocol.force_zero.store(identity, Ordering::SeqCst);
        drain_column();
        drain_blinds();
        take_stage_observations();
        let result = input.commit_quotient(1 << 26);
        if identity {
            assert!(matches!(&result, Err(StoredLookupErrorV1::Transcript)));
            assert_eq!(
                take_stage_observations().last(),
                Some(&StageEvent::WriteStart(0))
            );
        } else {
            let actual = result.as_ref().unwrap();
            assert_eq!(actual.commitments, expected);
            assert_eq!(*actual.x, x.unwrap());
            assert_eq!(
                actual.blinds.iter().map(|b| (b.0).0).collect::<Vec<_>>(),
                blinds.iter().map(|b| b.0).collect::<Vec<_>>()
            );
        }
        assert_eq!(
            shared.log.lock().unwrap().events,
            oracle_shared.log.lock().unwrap().events
        );
        assert_eq!(drain_column(), (6 * 16, true));
        drop(result);
        assert_eq!(drain_blinds(), (6, true));
        assert_eq!(storage.bank.lock().unwrap().live.len(), 1);
        check_sentinel(&mut other);
        drop(other);
        assert_dropped(&shared);
    }
}
#[test]
fn both_pasta_quotient_commitments_match_zero_coefficient_arithmetic_and_original_transcript_identity_rejection()
 {
    zero_and_identity::<EqAffine>();
    zero_and_identity::<EpAffine>();
}
