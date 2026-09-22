//! Stored multiopening parity against genuine dense PLONK queries and original IPA helpers.
//!
//! Dense fixture copies and diagnostic observers are test-only. A prepared P frontier and
//! test-only inner-IPA suffix do not claim a complete stored prover or production memory bound.

use super::*;
use crate::{
    plonk::prover::stored::proof_evaluations::opening::{
        self as opening, OpeningObservationV1 as Observation,
    },
    poly::{
        VerifierQuery,
        commitment::{MSM as _, Prover, Verifier},
        ipa::multiopen::{VerifierIPA, stored_oracle},
    },
    transcript::{Blake2bRead, EncodedChallenge, TranscriptRead, TranscriptReadBuffer},
};
use group::GroupEncoding;
use std::cell::RefCell;

#[derive(Clone)]
struct DenseSource {
    coefficient: Vec<[u8; 32]>,
    blind: [u8; 32],
}
#[derive(Clone)]
struct DenseCapture {
    sources: Vec<DenseSource>,
    queries: Vec<(usize, [u8; 32])>,
}
thread_local! {
    static DENSE: RefCell<Option<DenseCapture>> = const { RefCell::new(None) };
}
struct CaptureProver<'params, C: CurveAffine, const Q: bool, const M: u64>(&'params ParamsIPA<C>);
impl<'params, C: CurveAffine, const Q: bool, const M: u64> Prover<'params, IPACommitmentScheme<C>>
    for CaptureProver<'params, C, Q, M>
where
    C::Scalar: StoredAssignmentFieldV1,
{
    const QUERY_INSTANCE: bool = Q;
    const PROOF_SUPPLIED_INSTANCE_COMMITMENT_MASK: u64 = M;
    fn new(params: &'params ParamsIPA<C>) -> Self {
        Self(params)
    }
    fn create_proof<'com, E, T, R, I>(
        &self,
        rng: R,
        transcript: &mut T,
        queries: I,
    ) -> io::Result<()>
    where
        E: EncodedChallenge<C>,
        T: TranscriptWrite<C, E>,
        R: RngCore,
        I: IntoIterator<Item = ProverQuery<'com, C>> + Clone,
    {
        let queries = queries.into_iter().collect::<Vec<_>>();
        let mut pointers = Vec::new();
        let mut sources: Vec<DenseSource> = Vec::new();
        let mut rows = Vec::new();
        for query in &queries {
            let pointer = query.poly as *const Polynomial<C::Scalar, Coeff>;
            let source = pointers
                .iter()
                .position(|existing| *existing == pointer)
                .unwrap_or_else(|| {
                    pointers.push(pointer);
                    sources.push(DenseSource {
                        coefficient: query.poly.iter().map(PrimeField::to_repr).collect(),
                        blind: query.blind.0.to_repr(),
                    });
                    pointers.len() - 1
                });
            assert_eq!(sources[source].blind, query.blind.0.to_repr());
            rows.push((source, query.point.to_repr()));
        }
        DENSE.with(|capture| {
            *capture.borrow_mut() = Some(DenseCapture {
                sources,
                queries: rows,
            })
        });
        // Observe the ordinary caller's actual dense query/blind objects, then execute the
        // unchanged production multiopening prover. The stored blind bridge is not consulted.
        ProverIPA::<C, Q, M>::new(self.0).create_proof(rng, transcript, queries)
    }
}

struct Scripted<C: CurveAffine>(C::Scalar);
impl<C: CurveAffine> EncodedChallenge<C> for Scripted<C> {
    type Input = C::Scalar;
    fn new(value: &C::Scalar) -> Self {
        Self(*value)
    }
    fn get_scalar(&self) -> C::Scalar {
        self.0
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Boundary {
    Rng32,
    Rng64,
    Fill,
    TryFill,
    Challenge,
    Point,
    Scalar,
}
#[derive(Clone, Copy)]
enum OpeningFault {
    Error,
    Panic,
    Drift(u64),
}
struct OpeningLog<F> {
    active: bool,
    events: Vec<Boundary>,
    fault: Option<(usize, OpeningFault)>,
    scripted: Vec<F>,
    challenges: usize,
    force_next: Option<F>,
}
impl<F> Default for OpeningLog<F> {
    fn default() -> Self {
        Self {
            active: false,
            events: Vec::new(),
            fault: None,
            scripted: Vec::new(),
            challenges: 0,
            force_next: None,
        }
    }
}
struct OpeningControls<F> {
    log: Mutex<OpeningLog<F>>,
    storage: Arc<Controls>,
}
impl<F: Copy> OpeningControls<F> {
    fn new(storage: &Arc<Controls>) -> Arc<Self> {
        Arc::new(Self {
            log: Mutex::new(OpeningLog::default()),
            storage: Arc::clone(storage),
        })
    }
    fn before(&self, boundary: Boundary) -> Option<OpeningFault> {
        let mut log = self.log.lock().unwrap();
        if !log.active {
            return None;
        }
        let index = log.events.len();
        log.events.push(boundary);
        let fault = if log.fault.is_some_and(|(target, _)| target == index) {
            log.fault.take().map(|(_, fault)| fault)
        } else {
            None
        };
        drop(log);
        if matches!(fault, Some(OpeningFault::Panic)) {
            panic!("injected opening protocol unwind");
        }
        fault
    }
    fn after(&self, fault: Option<OpeningFault>) {
        if let Some(OpeningFault::Drift(ordinal)) = fault {
            let layout = *self
                .storage
                .bank
                .lock()
                .unwrap()
                .live
                .get(&ordinal)
                .unwrap();
            self.storage.after(Some(Action::Drift(ordinal)), layout);
        }
    }
}
struct OpeningRng<C: CurveAffine> {
    inner: Rng<C>,
    controls: Arc<OpeningControls<C::Scalar>>,
}
impl<C: CurveAffine> RngCore for OpeningRng<C> {
    fn next_u32(&mut self) -> u32 {
        let fault = self.controls.before(Boundary::Rng32);
        assert!(!matches!(fault, Some(OpeningFault::Error)));
        let value = self.inner.next_u32();
        self.controls.after(fault);
        value
    }
    fn next_u64(&mut self) -> u64 {
        let fault = self.controls.before(Boundary::Rng64);
        assert!(!matches!(fault, Some(OpeningFault::Error)));
        let value = self.inner.next_u64();
        self.controls.after(fault);
        value
    }
    fn fill_bytes(&mut self, output: &mut [u8]) {
        let fault = self.controls.before(Boundary::Fill);
        assert!(!matches!(fault, Some(OpeningFault::Error)));
        self.inner.fill_bytes(output);
        self.controls.after(fault);
    }
    fn try_fill_bytes(&mut self, output: &mut [u8]) -> Result<(), RngError> {
        let fault = self.controls.before(Boundary::TryFill);
        assert!(!matches!(fault, Some(OpeningFault::Error)));
        let result = self.inner.try_fill_bytes(output);
        self.controls.after(fault);
        result
    }
}
struct OpeningTranscript<C: CurveAffine>
where
    C::Scalar: FromUniformBytes<64>,
{
    inner: RecordingTranscript<C>,
    controls: Arc<OpeningControls<C::Scalar>>,
}
impl<C: CurveAffine> Transcript<C, Scripted<C>> for OpeningTranscript<C>
where
    C::Scalar: FromUniformBytes<64>,
{
    fn squeeze_challenge(&mut self) -> Scripted<C> {
        let fault = self.controls.before(Boundary::Challenge);
        assert!(!matches!(fault, Some(OpeningFault::Error)));
        let actual = self.inner.squeeze_challenge().get_scalar();
        let forced = {
            let mut log = self.controls.log.lock().unwrap();
            let explicit = log.force_next.take();
            if log.active {
                let index = log.challenges;
                log.challenges += 1;
                explicit.or_else(|| log.scripted.get(index).copied())
            } else {
                explicit
            }
        };
        let value = forced.unwrap_or(actual);
        if forced.is_some() {
            *self
                .inner
                .shared
                .log
                .lock()
                .unwrap()
                .events
                .last_mut()
                .unwrap() = Event::Challenge(value);
        }
        self.controls.after(fault);
        Scripted(value)
    }
    fn common_point(&mut self, point: C) -> io::Result<()> {
        self.inner.common_point(point)
    }
    fn common_scalar(&mut self, scalar: C::Scalar) -> io::Result<()> {
        self.inner.common_scalar(scalar)
    }
}
impl<C: CurveAffine> TranscriptWrite<C, Scripted<C>> for OpeningTranscript<C>
where
    C::Scalar: FromUniformBytes<64>,
{
    fn write_point(&mut self, point: C) -> io::Result<()> {
        let fault = self.controls.before(Boundary::Point);
        if matches!(fault, Some(OpeningFault::Error)) {
            return Err(io::Error::other("injected opening point error"));
        }
        self.inner.write_point(point)?;
        self.controls.after(fault);
        Ok(())
    }
    fn write_scalar(&mut self, value: C::Scalar) -> io::Result<()> {
        let fault = self.controls.before(Boundary::Scalar);
        if matches!(fault, Some(OpeningFault::Error)) {
            return Err(io::Error::other("injected opening scalar error"));
        }
        self.inner.write_scalar(value)?;
        self.controls.after(fault);
        Ok(())
    }
}
macro_rules! opening_coefficients {
    ($params:expr,$pk:expr,$instances:expr,$shared:expr,$storage:expr,$control:expr) => {
        prepare_single_phase_stored_ipa_prefix_v1::<C, _, _, _, Scripted<C>, _, Q, M>(
            $params,
            $pk,
            InverseCircuit::<C, DEGREE, MIXED, I>(Producer::new($shared, 3)),
            $instances,
            InverseProvider {
                inner: Provider::new($shared),
                controls: Arc::clone($storage),
            },
            OpeningRng {
                inner: Rng(Arc::clone($shared)),
                controls: Arc::clone($control),
            },
            OpeningTranscript {
                inner: RecordingTranscript::new($shared),
                controls: Arc::clone($control),
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
macro_rules! opening_input {
    ($params:expr,$pk:expr,$instances:expr,$shared:expr,$storage:expr,$control:expr) => {
        opening_coefficients!($params, $pk, $instances, $shared, $storage, $control)
            .commit_quotient(1 << 26)
            .unwrap()
            .evaluate_and_plan(1 << 26)
            .unwrap()
    };
}
fn bytes<F: StoredAssignmentFieldV1>(fields: &[F]) -> Vec<[u8; 32]> {
    fields.iter().map(PrimeField::to_repr).collect()
}
fn scalar<F: StoredAssignmentFieldV1>(value: [u8; 32]) -> F {
    Option::<F>::from(F::from_repr(value)).unwrap()
}

fn compare_observations<C: CurveAffine>(
    observations: &[Observation],
    oracle: &stored_oracle::Prepared<C>,
    plan: &[(Source, C::Scalar)],
) where
    C::Scalar: StoredAssignmentFieldV1,
{
    let mut challenges = Vec::new();
    let mut initial = Vec::new();
    let mut grouping = 0;
    let mut reconstructed = [0usize; 3];
    let mut divided = 0;
    let mut quotient = 0;
    let mut u = 0;
    let mut prepared = 0;
    for observation in observations {
        match observation {
            Observation::Challenge { number, value } => {
                challenges.push((*number, *value));
            }
            Observation::InitialEvaluation { query, value } => {
                initial.push((*query, *value));
            }
            Observation::Grouping {
                queries,
                sources,
                points,
                sets,
            } => {
                grouping += 1;
                assert_eq!(queries, &oracle.queries);
                assert_eq!(points, &bytes(&oracle.points));
                assert_eq!(sets, &oracle.sets);
                let expected = oracle
                    .sources
                    .iter()
                    .map(|source| {
                        (
                            plan[source.first_query].0,
                            source.first_query,
                            source.set,
                            source.points.clone(),
                            bytes(&source.evaluations),
                        )
                    })
                    .collect::<Vec<_>>();
                assert_eq!(sources, &expected);
            }
            Observation::Reconstructed {
                pass,
                set,
                coefficients,
                blind,
            } => {
                assert!(*pass < 3);
                assert_eq!(*set, reconstructed[*pass as usize]);
                reconstructed[*pass as usize] += 1;
                assert_eq!(coefficients, &bytes(&oracle.q[*set]));
                assert_eq!(*blind, oracle.q_blinds[*set].0.to_repr());
            }
            Observation::Divided { set, coefficients } => {
                assert_eq!(*set, divided);
                divided += 1;
                assert_eq!(coefficients, &bytes(&oracle.divided[*set]));
            }
            Observation::Quotient {
                coefficients,
                blind,
                commitment,
            } => {
                quotient += 1;
                assert_eq!(coefficients, &bytes(&oracle.q_prime));
                assert_eq!(*blind, oracle.q_prime_blind.0.to_repr());
                assert_eq!(commitment.as_slice(), oracle.commitment.to_bytes().as_ref());
            }
            Observation::U { set, value } => {
                assert_eq!(*set, u);
                u += 1;
                assert_eq!(*value, oracle.u[*set].to_repr());
            }
            Observation::Prepared {
                coefficients,
                blind,
                x3,
            } => {
                prepared += 1;
                assert_eq!(coefficients, &bytes(&oracle.p));
                assert_eq!(*blind, oracle.p_blind.0.to_repr());
                assert_eq!(*x3, oracle.challenges[2].to_repr());
            }
        }
    }
    assert_eq!(
        challenges,
        oracle
            .challenges
            .iter()
            .enumerate()
            .map(|(index, value)| ((index + 1) as u8, value.to_repr()))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        initial,
        oracle
            .evaluations
            .iter()
            .enumerate()
            .map(|(index, value)| (index, value.to_repr()))
            .collect::<Vec<_>>()
    );
    assert_eq!(grouping, 1);
    assert_eq!(reconstructed, [oracle.sets.len(); 3]);
    assert_eq!(divided, oracle.sets.len());
    assert_eq!(u, oracle.sets.len());
    assert_eq!(quotient, 1);
    assert_eq!(prepared, 1);
}

fn verify_suffix<C, const Q: bool, const M: u64>(
    params: &ParamsIPA<C>,
    proof: &[u8],
    prefix: &[Event<C>],
    queries: Vec<VerifierQuery<'_, C, crate::poly::ipa::msm::MSMIPA<'_, C>>>,
) -> bool
where
    C: CurveAffine,
    C::Scalar: FromUniformBytes<64>,
{
    let mut transcript = Blake2bRead::<_, C, Challenge255<C>>::init(proof);
    for event in prefix {
        let valid = match event {
            Event::CommonScalar(value) => transcript.common_scalar(*value).is_ok(),
            Event::CommonPoint(point) => transcript.common_point(*point).is_ok(),
            Event::WriteScalar(value) => transcript
                .read_scalar()
                .is_ok_and(|actual| actual == *value),
            Event::WritePoint(point) => {
                transcript.read_point().is_ok_and(|actual| actual == *point)
            }
            Event::Challenge(value) => transcript.squeeze_challenge().get_scalar() == *value,
        };
        if !valid {
            return false;
        }
    }
    VerifierIPA::<C, Q, M>::new(params)
        .verify_proof(&mut transcript, queries, params.empty_msm())
        .map(|guard| guard.use_challenges().check())
        .unwrap_or(false)
}

fn opening_success<
    C,
    const DEGREE: usize,
    const MIXED: bool,
    const I: usize,
    const Q: bool,
    const M: u64,
>(
    k: u32,
    empty: bool,
    script: Option<[C::Scalar; 4]>,
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
    let ordinary = Shared::<C>::new();
    let mut ordinary_transcript = RecordingTranscript::new(&ordinary);
    DENSE.with(|capture| *capture.borrow_mut() = None);
    create_proof_consuming::<
        IPACommitmentScheme<C>,
        CaptureProver<'_, C, Q, M>,
        Challenge255<C>,
        _,
        _,
        _,
    >(
        &params,
        pk.clone(),
        InverseCircuit::<C, DEGREE, MIXED, I>(Producer::new(&ordinary, 3)),
        &[&instances],
        Rng(Arc::clone(&ordinary)),
        &mut ordinary_transcript,
    )
    .unwrap();
    let capture = DENSE.with(|capture| capture.borrow_mut().take().unwrap());
    let polys = capture
        .sources
        .iter()
        .map(|source| {
            pk.vk.domain.coeff_from_vec(
                source
                    .coefficient
                    .iter()
                    .map(|value| scalar::<C::Scalar>(*value))
                    .collect(),
            )
        })
        .collect::<Vec<_>>();
    let blinds = capture
        .sources
        .iter()
        .map(|source| Blind(scalar::<C::Scalar>(source.blind)))
        .collect::<Vec<_>>();
    let queries = capture
        .queries
        .iter()
        .map(|(source, point)| ProverQuery::<C> {
            point: scalar::<C::Scalar>(*point),
            poly: &polys[*source],
            blind: blinds[*source],
        })
        .collect::<Vec<_>>();
    let shared = Shared::<C>::new();
    let storage = Controls::new();
    let control = OpeningControls::new(&storage);
    let mut coefficient =
        opening_coefficients!(&params, pk, &instances, &shared, &storage, &control);
    let mut other = sentinel(&mut coefficient.inner.provider, k);
    let input = coefficient
        .commit_quotient(1 << 26)
        .unwrap()
        .evaluate_and_plan(1 << 26)
        .unwrap();
    let plan = input
        .opening_plan()
        .unwrap()
        .iter()
        .map(|query| (query.source(), query.point()))
        .collect::<Vec<_>>();
    assert_eq!(plan.len(), queries.len());
    for ((_, point), query) in plan.iter().zip(&queries) {
        assert_eq!(*point, query.point);
    }
    let prefix = shared.log.lock().unwrap().events.clone();
    assert_eq!(prefix, ordinary.log.lock().unwrap().events[..prefix.len()]);
    let mut oracle_rng = shared.rng.lock().unwrap().clone();
    let oracle_shared = Shared::<C>::new();
    oracle_shared.log.lock().unwrap().events = prefix.clone();
    let oracle_control = OpeningControls::new(&Controls::new());
    {
        let mut log = oracle_control.log.lock().unwrap();
        log.active = true;
        log.scripted = script.map(Vec::from).unwrap_or_default();
    }
    let mut oracle_transcript = OpeningTranscript {
        inner: RecordingTranscript {
            inner: input
                .observed_inner()
                .inner
                .inner
                .transcript
                .inner
                .inner
                .clone(),
            shared: Arc::clone(&oracle_shared),
        },
        controls: oracle_control,
    };
    let expected = stored_oracle::prepare(
        &params,
        &mut oracle_rng,
        &mut oracle_transcript,
        queries.clone(),
    )
    .unwrap();
    let originals = storage.bank.lock().unwrap().live.clone();
    let cursor = input.observed_inner().inner.inner.provider.inner.ordinal;
    let limit = opening::scratch_bytes(&input).unwrap();
    storage.arm(None);
    {
        let mut log = control.log.lock().unwrap();
        log.active = true;
        log.scripted = script.map(Vec::from).unwrap_or_default();
    }
    opening::take_observations();
    evaluations::take_clear_observations();
    let actual = input.prepare_ipa_opening(limit).unwrap();
    compare_observations::<C>(&opening::take_observations(), &expected, &plan);
    assert_eq!(actual.observed_p().to_vec(), expected.p.to_vec());
    assert_eq!(actual.observed_p_blind().0, expected.p_blind.0);
    assert_eq!(*actual.observed_x3(), *expected.x3);
    assert_eq!(
        shared.log.lock().unwrap().events,
        oracle_shared.log.lock().unwrap().events
    );
    let mut next_actual = [0; 64];
    let mut next_expected = [0; 64];
    shared
        .rng
        .lock()
        .unwrap()
        .clone()
        .fill_bytes(&mut next_actual);
    oracle_rng.clone().fill_bytes(&mut next_expected);
    assert_eq!(next_actual, next_expected);
    assert_eq!(storage.bank.lock().unwrap().live, originals);
    assert_eq!(
        actual
            .observed_inner()
            .observed_inner()
            .inner
            .inner
            .provider
            .inner
            .ordinal,
        cursor
    );
    assert!(
        storage
            .bank
            .lock()
            .unwrap()
            .events
            .iter()
            .all(|event| event.kind == IoKind::Read)
    );
    let (cleared, zero) = evaluations::take_clear_observations();
    assert!(cleared > 2 * params.n() as usize && zero);
    // Test the prepared values with the real unchanged inner IPA and the original protocol
    // owners. This deliberately test-only adapter is not a stored completion API.
    if script.is_none() {
        let finished = actual.finish_ordinary_ipa_for_test().unwrap();
        let proof = finished
            .observed_inner()
            .inner
            .inner
            .transcript
            .inner
            .inner
            .clone()
            .finalize();
        assert_eq!(proof, ordinary_transcript.inner.clone().finalize());
        let mut ordinary_next = [0; 64];
        ordinary
            .rng
            .lock()
            .unwrap()
            .clone()
            .fill_bytes(&mut ordinary_next);
        shared
            .rng
            .lock()
            .unwrap()
            .clone()
            .fill_bytes(&mut next_actual);
        assert_eq!(next_actual, ordinary_next);
        let commitments = polys
            .iter()
            .zip(&blinds)
            .map(|(poly, blind)| params.commit(poly, *blind).to_affine())
            .collect::<Vec<_>>();
        let verifier_queries = capture
            .queries
            .iter()
            .map(|(source, point)| {
                let point = scalar::<C::Scalar>(*point);
                VerifierQuery::new_commitment(
                    &commitments[*source],
                    point,
                    eval_polynomial(&polys[*source], point),
                )
            })
            .collect::<Vec<_>>();
        assert!(verify_suffix::<C, Q, M>(
            &params,
            &proof,
            &prefix,
            verifier_queries.clone()
        ));
        let mut altered = verifier_queries.clone();
        altered[0].eval += C::Scalar::ONE;
        assert!(!verify_suffix::<C, Q, M>(&params, &proof, &prefix, altered));
        let mut altered = verifier_queries.clone();
        altered[0].point += C::Scalar::ONE;
        assert!(!verify_suffix::<C, Q, M>(&params, &proof, &prefix, altered));
        let wrong_commitment =
            (commitments[capture.queries[0].0].to_curve() + params.get_blind_base()).to_affine();
        let mut altered = verifier_queries.clone();
        altered[0] =
            VerifierQuery::new_commitment(&wrong_commitment, altered[0].point, altered[0].eval);
        assert!(!verify_suffix::<C, Q, M>(&params, &proof, &prefix, altered));
        let mut corrupted = proof.clone();
        *corrupted.last_mut().unwrap() ^= 1;
        assert!(!verify_suffix::<C, Q, M>(
            &params,
            &corrupted,
            &prefix,
            verifier_queries.clone()
        ));
        assert!(!verify_suffix::<C, Q, M>(
            &params,
            &proof[..proof.len() - 1],
            &prefix,
            verifier_queries
        ));
        drop(finished);
    } else {
        drop(actual);
    }
    assert_eq!(storage.bank.lock().unwrap().live.len(), 1);
    check_sentinel(&mut other);
    drop(other);
    assert_dropped(&shared);
}

fn opening_matrix<C>()
where
    C: CurveAffine + crate::SerdeCurveAffine,
    C::Scalar: StoredAssignmentFieldV1
        + WithSmallOrderMulGroup<3>
        + FromUniformBytes<64>
        + crate::SerdePrimeField,
{
    opening_success::<C, 4, true, 4, false, 0>(4, false, None);
    opening_success::<C, 4, true, 4, true, 0>(4, false, None);
    opening_success::<C, 4, true, 4, true, 6>(4, true, None);
    opening_success::<C, 3, false, 0, false, 0>(8, true, None);
    opening_success::<C, 7, false, 1, true, 0>(9, true, None);
    for value in [
        C::Scalar::ZERO,
        C::Scalar::ONE,
        -C::Scalar::ONE,
        C::Scalar::from(19),
    ] {
        opening_success::<C, 4, true, 4, true, 6>(
            4,
            false,
            Some([value, value, C::Scalar::from(31), value]),
        );
    }
}
#[test]
fn both_pasta_stored_opening_matches_genuine_dense_planner_q_quotient_p_and_full_ipa_suffix() {
    opening_matrix::<EqAffine>();
    opening_matrix::<EpAffine>();
}

fn planner_matrix<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(4);
    let domain = EvaluationDomain::new(3, 4);
    // Distinct polynomial objects deliberately have equal values and equal blinds. Identity
    // must follow the objects, not coefficients, source discriminant sorting or commitments.
    let polys = (0..6)
        .map(|source| {
            domain.coeff_from_vec(
                (0..16)
                    .map(|row| C::Scalar::from((source / 2 * 13 + row + 1) as u64))
                    .collect(),
            )
        })
        .collect::<Vec<_>>();
    let point_values = [5, 2, 13, 7].map(C::Scalar::from);
    let base = vec![
        (0, 0),
        (0, 1),
        (1, 0),
        (1, 1),
        (2, 2),
        (3, 0),
        (3, 2),
        (4, 0),
        (4, 2),
        (5, 1),
        (5, 2),
        (5, 3),
    ];
    for order in 0..3 {
        let mut rows = base.clone();
        if order == 1 {
            rows.reverse();
        }
        if order == 2 {
            rows.rotate_left(5);
        }
        let logical = rows
            .iter()
            .map(|(source, point)| (Source::Advice(*source), point_values[*point]))
            .collect::<Vec<_>>();
        let dense = rows
            .iter()
            .map(|(source, point)| ProverQuery::<C> {
                point: point_values[*point],
                poly: &polys[*source],
                blind: Blind(C::Scalar::from(11)),
            })
            .collect::<Vec<_>>();
        let shared = Shared::<C>::new();
        let oracle = stored_oracle::prepare(
            &params,
            ChaCha20Rng::from_seed([29; 32]),
            &mut RecordingTranscript::new(&shared),
            dense,
        )
        .unwrap();
        let Observation::Grouping {
            queries,
            sources,
            points,
            sets,
        } = opening::project_plan_for_test(&logical).unwrap()
        else {
            panic!("planner returned a non-grouping observation");
        };
        assert_eq!(queries, oracle.queries);
        assert_eq!(points, bytes(&oracle.points));
        assert_eq!(sets, oracle.sets);
        assert_eq!(sources.len(), 6);
        for (actual, expected) in sources.iter().zip(&oracle.sources) {
            assert_eq!(
                (actual.0, actual.1, actual.2),
                (
                    logical[expected.first_query].0,
                    expected.first_query,
                    expected.set
                )
            );
            assert_eq!(actual.3, expected.points);
            assert_eq!(
                actual.4,
                vec![C::Scalar::ZERO.to_repr(); expected.points.len()]
            );
        }
        let mut duplicate = logical.clone();
        duplicate.push(logical[0]);
        assert!(opening::project_plan_for_test(&duplicate).is_err());
    }
    for evaluated in [[C::Scalar::ONE; 2], [C::Scalar::ONE, C::Scalar::ZERO]] {
        assert_eq!(
            stored_oracle::duplicate_evaluations_rejected(C::Scalar::ZERO, evaluated),
            (true, 0)
        );
    }
}
#[test]
fn both_pasta_stored_planner_preserves_first_encounter_identity_and_rejects_duplicate_evaluations()
{
    planner_matrix::<EqAffine>();
    planner_matrix::<EpAffine>();
}

fn collision<C>()
where
    C: CurveAffine + crate::SerdeCurveAffine,
    C::Scalar: StoredAssignmentFieldV1
        + WithSmallOrderMulGroup<3>
        + FromUniformBytes<64>
        + crate::SerdePrimeField,
{
    const DEGREE: usize = 4;
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
    let control = OpeningControls::new(&storage);
    let mut coefficient =
        opening_coefficients!(&params, pk.clone(), &instances, &shared, &storage, &control);
    let mut other = sentinel(&mut coefficient.inner.provider, 4);
    // Script the real quotient-commitment transcript callback, not the returned plan/owner.
    control.log.lock().unwrap().force_next = Some(C::Scalar::ZERO);
    let input = coefficient
        .commit_quotient(1 << 26)
        .unwrap()
        .evaluate_and_plan(1 << 26)
        .unwrap();
    let calls = shared.log.lock().unwrap().rng_calls;
    let before = shared.log.lock().unwrap().events.len();
    let limit = opening::scratch_bytes(&input).unwrap();
    storage.arm(None);
    control.log.lock().unwrap().active = true;
    opening::take_observations();
    evaluations::take_clear_observations();
    assert!(input.prepare_ipa_opening(limit).is_err());
    assert_eq!(
        control.log.lock().unwrap().events,
        [Boundary::Challenge, Boundary::Challenge]
    );
    assert_eq!(shared.log.lock().unwrap().rng_calls, calls);
    assert_eq!(shared.log.lock().unwrap().events.len(), before + 2);
    assert!(
        storage
            .bank
            .lock()
            .unwrap()
            .events
            .iter()
            .all(|event| matches!(event.kind, IoKind::DropSnapshot | IoKind::DropWriter))
    );
    assert!(
        opening::take_observations()
            .iter()
            .all(|observation| matches!(observation, Observation::Challenge { .. }))
    );
    assert!(evaluations::take_clear_observations().1);
    assert_eq!(storage.bank.lock().unwrap().live.len(), 1);
    check_sentinel(&mut other);
    drop(other);
    assert_dropped(&shared);
    let ordinary = Shared::<C>::new();
    let ordinary_storage = Controls::new();
    let ordinary_control = OpeningControls::new(&ordinary_storage);
    ordinary_control.log.lock().unwrap().active = true;
    let mut transcript = OpeningTranscript {
        inner: RecordingTranscript::new(&ordinary),
        controls: Arc::clone(&ordinary_control),
    };
    let query = ProverQuery::<C> {
        point: C::Scalar::ZERO,
        poly: &pk.fixed_polys[0],
        blind: Blind::default(),
    };
    assert!(
        stored_oracle::prepare(
            &params,
            Rng(Arc::clone(&ordinary)),
            &mut transcript,
            vec![query.clone(), query]
        )
        .is_err()
    );
    assert_eq!(
        ordinary_control.log.lock().unwrap().events,
        [Boundary::Challenge, Boundary::Challenge]
    );
    assert_eq!(ordinary.log.lock().unwrap().rng_calls, 0);
}
#[test]
fn both_pasta_zero_plonk_challenge_collision_rejects_after_x1_x2_before_reads_or_randomness() {
    collision::<EqAffine>();
    collision::<EpAffine>();
}

#[derive(Clone, Copy)]
enum Failure {
    Budget(bool),
    Read(usize, Action),
    Protocol(usize, OpeningFault),
    InitialDrift,
}
fn opening_failures<C>(protocol_failures: bool)
where
    C: CurveAffine + crate::SerdeCurveAffine,
    C::Scalar: StoredAssignmentFieldV1
        + WithSmallOrderMulGroup<3>
        + FromUniformBytes<64>
        + crate::SerdePrimeField,
{
    const DEGREE: usize = 4;
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
    let control = OpeningControls::new(&storage);
    let input = opening_input!(&params, pk.clone(), &instances, &shared, &storage, &control);
    storage.arm(None);
    control.log.lock().unwrap().active = true;
    opening::take_observations();
    let actual = input.prepare_ipa_opening(1 << 26).unwrap();
    let reads = storage.bank.lock().unwrap().events.len();
    let events = control.log.lock().unwrap().events.clone();
    assert!(reads > 10);
    assert!(events.contains(&Boundary::Point));
    assert_eq!(
        events
            .iter()
            .filter(|kind| **kind == Boundary::Challenge)
            .count(),
        4
    );
    drop(actual);
    assert_dropped(&shared);
    opening::take_observations();
    let mut failures = Vec::new();
    if protocol_failures {
        for (index, boundary) in events.iter().enumerate() {
            failures.push(Failure::Protocol(index, OpeningFault::Panic));
            if matches!(boundary, Boundary::Point | Boundary::Scalar) {
                failures.push(Failure::Protocol(index, OpeningFault::Error));
            }
        }
        for index in [0, events.len() / 2, events.len() - 1] {
            failures.push(Failure::Protocol(index, OpeningFault::Drift(0)));
        }
    } else {
        failures.extend([
            Failure::Budget(true),
            Failure::Budget(false),
            Failure::InitialDrift,
        ]);
        // Every actual source read in the complete successful schedule receives a failing
        // backend result once. Additional malformed/unwind cases target early/middle/late reads.
        failures.extend((0..reads).map(|index| Failure::Read(index, Action::Error)));
        for index in [0, reads / 2, reads - 1] {
            for action in [
                Action::Panic,
                Action::Short,
                Action::Long,
                Action::Encoding,
                Action::Drift(0),
            ] {
                failures.push(Failure::Read(index, action));
            }
        }
    }
    for failure in failures {
        let shared = Shared::<C>::new();
        let storage = Controls::new();
        let control = OpeningControls::new(&storage);
        let mut coefficient =
            opening_coefficients!(&params, pk.clone(), &instances, &shared, &storage, &control);
        let victim = coefficient.pieces.last().unwrap().layout;
        let mut other = sentinel(&mut coefficient.inner.provider, 4);
        let input = coefficient
            .commit_quotient(1 << 26)
            .unwrap()
            .evaluate_and_plan(1 << 26)
            .unwrap();
        let minimum = opening::scratch_bytes(&input).unwrap();
        let calls = shared.log.lock().unwrap().rng_calls;
        let before = shared.log.lock().unwrap().events.clone();
        storage.arm(None);
        control.log.lock().unwrap().active = true;
        let mut budget = minimum;
        let mut panics = false;
        match failure {
            Failure::Budget(zero) => budget = if zero { 0 } else { minimum - 1 },
            Failure::InitialDrift => storage.after(Some(Action::Drift(victim.ordinal())), victim),
            Failure::Read(index, mut action) => {
                panics = action == Action::Panic;
                if matches!(action, Action::Drift(_)) {
                    action = Action::Drift(victim.ordinal());
                }
                storage.bank.lock().unwrap().fault = Some((index, action));
            }
            Failure::Protocol(index, mut action) => {
                panics = matches!(action, OpeningFault::Panic);
                if matches!(action, OpeningFault::Drift(_)) {
                    action = OpeningFault::Drift(victim.ordinal());
                }
                control.log.lock().unwrap().fault = Some((index, action));
            }
        }
        opening::take_observations();
        evaluations::take_clear_observations();
        drain_blinds();
        let result = catch_unwind(AssertUnwindSafe(|| input.prepare_ipa_opening(budget)));
        if panics {
            assert!(result.is_err());
        } else {
            assert!(matches!(&result, Ok(Err(_))));
        }
        drop(result);
        assert_eq!(storage.bank.lock().unwrap().live.len(), 1);
        match failure {
            Failure::Budget(_) | Failure::InitialDrift => {
                assert!(control.log.lock().unwrap().events.is_empty());
                assert_eq!(shared.log.lock().unwrap().rng_calls, calls);
                assert_eq!(shared.log.lock().unwrap().events, before);
                assert!(
                    storage
                        .bank
                        .lock()
                        .unwrap()
                        .events
                        .iter()
                        .all(|event| matches!(
                            event.kind,
                            IoKind::DropSnapshot | IoKind::DropWriter
                        ))
                );
            }
            Failure::Read(index, _) => {
                assert!(storage.bank.lock().unwrap().fault.is_none());
                assert_eq!(
                    storage
                        .bank
                        .lock()
                        .unwrap()
                        .events
                        .iter()
                        .filter(|event| event.kind == IoKind::Read)
                        .count(),
                    index + 1
                );
            }
            Failure::Protocol(index, action) => {
                assert!(control.log.lock().unwrap().fault.is_none());
                let sampling = |boundary: Boundary| {
                    matches!(
                        boundary,
                        Boundary::Rng32 | Boundary::Rng64 | Boundary::Fill | Boundary::TryFill
                    )
                };
                if matches!(action, OpeningFault::Drift(_)) && sampling(events[index]) {
                    // Field::random is one atomic primitive bracketed by the owner sweeps.
                    // Both Pasta fields request eight next_u64 values in that one sample;
                    // metadata drift cannot abort its infallible internal RNG subcalls.
                    let end = events[index + 1..]
                        .iter()
                        .position(|boundary| !sampling(*boundary))
                        .map(|offset| index + 1 + offset)
                        .unwrap_or(events.len());
                    assert_eq!(control.log.lock().unwrap().events, events[..end]);
                    assert!(
                        opening::take_observations()
                            .iter()
                            .all(|observation| !matches!(
                                observation,
                                Observation::Quotient { .. }
                                    | Observation::U { .. }
                                    | Observation::Prepared { .. }
                                    | Observation::Challenge { number: 3 | 4, .. }
                            ))
                    );
                } else {
                    assert_eq!(control.log.lock().unwrap().events, events[..index + 1]);
                }
            }
        }
        assert!(
            storage
                .bank
                .lock()
                .unwrap()
                .events
                .iter()
                .all(|event| matches!(
                    event.kind,
                    IoKind::Read | IoKind::DropSnapshot | IoKind::DropWriter
                ))
        );
        let (cleared, zero) = evaluations::take_clear_observations();
        assert!(zero);
        if !matches!(failure, Failure::Budget(_) | Failure::InitialDrift) {
            assert!(cleared > 0);
        }
        let (count, zero) = drain_blinds();
        assert!(count > 0 && zero);
        opening::take_observations();
        check_sentinel(&mut other);
        drop(other);
        assert_dropped(&shared);
    }
}
#[test]
fn both_pasta_stored_opening_all_observed_reads_and_budget_failures_destroy_original_owners() {
    opening_failures::<EqAffine>(false);
    opening_failures::<EpAffine>(false);
}
#[test]
fn both_pasta_stored_opening_all_protocol_unwinds_and_write_errors_stop_and_clear() {
    opening_failures::<EqAffine>(true);
    opening_failures::<EpAffine>(true);
}

#[path = "satisfiable_tests.rs"]
mod satisfiable;
