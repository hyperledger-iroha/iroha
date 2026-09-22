//! Actual ordinary argument evaluation/opening oracles for the consuming stored continuation.
//!
//! The retained plaintext test backend checks ownership, arithmetic and transcript ordering;
//! it does not qualify encrypted production storage, a complete proof or process memory.

use super::*;
use crate::{
    arithmetic::eval_polynomial,
    plonk::prover::stored::proof_evaluations::{
        self as evaluations, StoredOpeningBlindV1 as OpeningBlind, StoredOpeningSourceV1 as Source,
    },
    poly::{Coeff, Polynomial, query::ProverQuery},
};

#[derive(Clone, Copy)]
enum ScalarFault {
    Error,
    Panic,
    Drift(u64),
}
#[derive(Default)]
struct ScalarLog {
    active: bool,
    calls: usize,
    fault: Option<(usize, ScalarFault)>,
    bytes: Vec<u8>,
}
struct EvaluationTranscript<C: CurveAffine>
where
    C::Scalar: FromUniformBytes<64>,
{
    inner: RecordingTranscript<C>,
    storage: Arc<Controls>,
    scalar: Arc<Mutex<ScalarLog>>,
}
impl<C: CurveAffine> Transcript<C, Challenge255<C>> for EvaluationTranscript<C>
where
    C::Scalar: FromUniformBytes<64>,
{
    fn squeeze_challenge(&mut self) -> Challenge255<C> {
        assert!(
            !self.scalar.lock().unwrap().active,
            "unexpected evaluation challenge"
        );
        self.inner.squeeze_challenge()
    }
    fn common_point(&mut self, point: C) -> io::Result<()> {
        assert!(
            !self.scalar.lock().unwrap().active,
            "unexpected evaluation common point"
        );
        self.inner.common_point(point)
    }
    fn common_scalar(&mut self, scalar: C::Scalar) -> io::Result<()> {
        assert!(
            !self.scalar.lock().unwrap().active,
            "unexpected evaluation common scalar"
        );
        self.inner.common_scalar(scalar)
    }
}
impl<C: CurveAffine> TranscriptWrite<C, Challenge255<C>> for EvaluationTranscript<C>
where
    C::Scalar: FromUniformBytes<64>,
{
    fn write_point(&mut self, point: C) -> io::Result<()> {
        assert!(
            !self.scalar.lock().unwrap().active,
            "unexpected evaluation point write"
        );
        self.inner.write_point(point)
    }
    fn write_scalar(&mut self, value: C::Scalar) -> io::Result<()> {
        let fault = {
            let mut scalar = self.scalar.lock().unwrap();
            if scalar.active {
                let index = scalar.calls;
                scalar.calls += 1;
                if scalar.fault.is_some_and(|(target, _)| target == index) {
                    scalar.fault.take().map(|(_, fault)| fault)
                } else {
                    None
                }
            } else {
                None
            }
        };
        match fault {
            Some(ScalarFault::Panic) => panic!("injected scalar continuation unwind"),
            Some(ScalarFault::Error) => return Err(io::Error::other("injected scalar write")),
            _ => (),
        }
        self.inner.write_scalar(value)?;
        self.scalar.lock().unwrap().bytes = self.inner.inner.clone().finalize();
        if let Some(ScalarFault::Drift(ordinal)) = fault {
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
        Ok(())
    }
}

macro_rules! evaluation_member {
    ($params:expr,$pk:expr,$instances:expr,$shared:expr,$storage:expr,$scalar:expr) => {
        prepare_single_phase_stored_ipa_prefix_v1::<C, _, _, _, Challenge255<C>, _, Q, M>(
            $params,
            $pk,
            InverseCircuit::<C, DEGREE, MIXED, I>(Producer::new($shared, 3)),
            $instances,
            InverseProvider {
                inner: Provider::new($shared),
                controls: Arc::clone($storage),
            },
            QuietRng {
                inner: Rng(Arc::clone($shared)),
                controls: Arc::clone($storage),
            },
            EvaluationTranscript {
                inner: RecordingTranscript::new($shared),
                storage: Arc::clone($storage),
                scalar: Arc::clone($scalar),
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
macro_rules! evaluation_input {
    ($params:expr,$pk:expr,$instances:expr,$shared:expr,$storage:expr,$scalar:expr) => {
        evaluation_member!($params, $pk, $instances, $shared, $storage, $scalar)
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
            .commit_quotient(1 << 26)
            .unwrap()
    };
}

struct OracleQuery<F> {
    source: Source,
    point: F,
    coefficient: Vec<F>,
    // Advice blinds are deliberately symbolic: the phase owner keeps its sole private guard.
    blind: Option<F>,
}
struct EvaluationOracle<C: CurveAffine> {
    queries: Vec<OracleQuery<C::Scalar>>,
    events_before: Vec<Event<C>>,
    events_after: Vec<Event<C>>,
    bytes: Vec<u8>,
    rng_next: [u8; 64],
}
fn append_query<C: CurveAffine>(
    output: &mut Vec<OracleQuery<C::Scalar>>,
    source: Source,
    query: ProverQuery<'_, C>,
) {
    output.push(OracleQuery {
        source,
        point: query.point,
        coefficient: query.poly.to_vec(),
        blind: Some(query.blind.0),
    });
}

fn ordinary_evaluations<C, const Q: bool>(
    pk: &ProvingKey<C>,
    params: &ParamsIPA<C>,
    theta: crate::plonk::ChallengeTheta<C>,
    advice: &[Polynomial<C::Scalar, crate::poly::LagrangeCoeff>],
    instance: &[Polynomial<C::Scalar, crate::poly::LagrangeCoeff>],
    challenges: &[C::Scalar],
    mut rng: ChaCha20Rng,
    mut transcript: RecordingTranscript<C>,
) -> EvaluationOracle<C>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    // Fork the genuine completed prefix; retain the real ordinary argument owners through
    // evaluate/open. No constructed/evaluated argument or expected coefficient bank is forged.
    let domain = &pk.vk.domain;
    let pairs = pk
        .vk
        .cs
        .lookups
        .iter()
        .map(|argument| {
            argument
                .commit_permuted(
                    pk,
                    params,
                    domain,
                    theta,
                    advice,
                    &pk.fixed_values,
                    instance,
                    challenges,
                    &mut rng,
                    &mut transcript,
                )
                .unwrap()
        })
        .collect::<Vec<_>>();
    let beta: ChallengeBeta<C> = transcript.squeeze_challenge_scalar();
    let gamma: ChallengeGamma<C> = transcript.squeeze_challenge_scalar();
    let permutation = pk
        .vk
        .cs
        .permutation
        .commit(
            params,
            pk,
            &pk.permutation,
            advice,
            &pk.fixed_values,
            instance,
            beta,
            gamma,
            &mut rng,
            &mut transcript,
        )
        .unwrap();
    let lookups = pairs
        .into_iter()
        .map(|pair| {
            pair.commit_product(pk, params, beta, gamma, &mut rng, &mut transcript)
                .unwrap()
        })
        .collect::<Vec<_>>();
    let vanishing =
        crate::plonk::vanishing::Argument::<C>::commit(params, domain, &mut rng, &mut transcript)
            .unwrap();
    let y: ChallengeY<C> = transcript.squeeze_challenge_scalar();
    let advice = advice
        .iter()
        .map(|v| domain.lagrange_to_coeff(v.clone()))
        .collect::<Vec<_>>();
    let instance = instance
        .iter()
        .map(|v| domain.lagrange_to_coeff(v.clone()))
        .collect::<Vec<_>>();
    let permutations = vec![permutation];
    let lookups = vec![lookups];
    let numerator = pk.ev.evaluate_h(
        pk,
        &[advice.as_slice()],
        &[instance.as_slice()],
        challenges,
        *y,
        *beta,
        *gamma,
        *theta,
        &lookups,
        &permutations,
        false,
    );
    let vanishing = vanishing
        .construct(params, domain, numerator, &mut rng, &mut transcript)
        .unwrap();
    let x: ChallengeX<C> = transcript.squeeze_challenge_scalar();
    let events_before = transcript.shared.log.lock().unwrap().events.clone();
    let mut queries = Vec::new();
    if Q {
        for &(column, rotation) in &pk.vk.cs.instance_queries {
            transcript
                .write_scalar(eval_polynomial(
                    &instance[column.index()],
                    domain.rotate_omega(*x, rotation),
                ))
                .unwrap();
            append_query(
                &mut queries,
                Source::Instance(column.index()),
                ProverQuery::<C> {
                    point: domain.rotate_omega(*x, rotation),
                    poly: &instance[column.index()],
                    blind: Blind::default(),
                },
            );
        }
    }
    for &(column, rotation) in &pk.vk.cs.advice_queries {
        let point = domain.rotate_omega(*x, rotation);
        transcript
            .write_scalar(eval_polynomial(&advice[column.index()], point))
            .unwrap();
        queries.push(OracleQuery {
            source: Source::Advice(column.index()),
            point,
            coefficient: advice[column.index()].to_vec(),
            blind: None,
        });
    }
    for &(column, rotation) in &pk.vk.cs.fixed_queries {
        transcript
            .write_scalar(eval_polynomial(
                &pk.fixed_polys[column.index()],
                domain.rotate_omega(*x, rotation),
            ))
            .unwrap();
    }
    let vanishing = vanishing
        .evaluate(x, x.pow([params.n()]), domain, &mut transcript)
        .unwrap();
    pk.permutation.evaluate(x, &mut transcript).unwrap();
    let permutation = permutations
        .into_iter()
        .next()
        .unwrap()
        .construct()
        .evaluate(pk, x, &mut transcript)
        .unwrap();
    let lookups = lookups
        .into_iter()
        .next()
        .unwrap()
        .into_iter()
        .map(|lookup| lookup.evaluate(pk, x, &mut transcript).unwrap())
        .collect::<Vec<_>>();
    // Derive source identity from ordinary polynomial pointer identity, independently of
    // stored query enumeration. Copy-product open reverses the final-row subset.
    let mut copy_pointers = Vec::<*const Polynomial<C::Scalar, Coeff>>::new();
    for query in permutation.open(pk, x) {
        let pointer = query.poly as *const _;
        let index = copy_pointers
            .iter()
            .position(|existing| *existing == pointer)
            .unwrap_or_else(|| {
                copy_pointers.push(pointer);
                copy_pointers.len() - 1
            });
        append_query(&mut queries, Source::CopyProduct(index), query);
    }
    for (index, lookup) in lookups.iter().enumerate() {
        let sources = [
            Source::LookupProduct(index),
            Source::LookupInput(index),
            Source::LookupTable(index),
            Source::LookupInput(index),
            Source::LookupProduct(index),
        ];
        for (query, source) in lookup.open(pk, x).zip(sources) {
            append_query(&mut queries, source, query);
        }
    }
    for &(column, rotation) in &pk.vk.cs.fixed_queries {
        append_query(
            &mut queries,
            Source::Fixed(column.index()),
            ProverQuery::<C> {
                point: domain.rotate_omega(*x, rotation),
                poly: &pk.fixed_polys[column.index()],
                blind: Blind::default(),
            },
        );
    }
    for (index, query) in pk.permutation.open(x).enumerate() {
        append_query(&mut queries, Source::Permutation(index), query);
    }
    for (query, source) in vanishing.open(x).zip([Source::Quotient, Source::Random]) {
        append_query(&mut queries, source, query);
    }
    let mut rng_next = [0; 64];
    rng.fill_bytes(&mut rng_next);
    let events_after = transcript.shared.log.lock().unwrap().events.clone();
    EvaluationOracle {
        queries,
        events_before,
        events_after,
        bytes: transcript.inner.clone().finalize(),
        rng_next,
    }
}

fn evaluation_success<
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
    let limit = evaluations::scratch_bytes::<C, InverseProvider<C>, Q, M>(&pk).unwrap();
    let values = values::<C, I>(empty);
    let instance = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let shared = Shared::<C>::new();
    let storage = Controls::new();
    let scalar = Arc::new(Mutex::new(ScalarLog::default()));
    let member = evaluation_member!(&params, pk, &instance, &shared, &storage, &scalar);
    let n = params.n() as usize;
    let domain = &member.compressed.inner.pk.vk.domain;
    let advice = member
        .compressed
        .inner
        .advice
        .layouts()
        .unwrap()
        .map(|layout| domain.lagrange_from_vec(stored_values::<C>(&shared, layout)))
        .collect::<Vec<_>>();
    let dense_instance = values
        .iter()
        .map(|column| {
            let mut column = column.clone();
            column.resize(n, C::Scalar::ZERO);
            domain.lagrange_from_vec(column)
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
    let expected = ordinary_evaluations::<C, Q>(
        &member.compressed.inner.pk,
        &params,
        member.compressed.theta,
        &advice,
        &dense_instance,
        &challenges,
        shared.rng.lock().unwrap().clone(),
        RecordingTranscript {
            inner: member.compressed.inner.transcript.inner.inner.clone(),
            shared: oracle_shared,
        },
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
        .unwrap()
        .commit_quotient(1 << 26)
        .unwrap();
    assert_eq!(shared.log.lock().unwrap().events, expected.events_before);
    let mut other = sentinel(&mut input.inner.inner.provider, k);
    let originals = storage.bank.lock().unwrap().live.clone();
    let cursor = input.inner.inner.provider.inner.ordinal;
    let calls = shared.log.lock().unwrap().rng_calls;
    let pk_pointer = input.inner.inner.pk.fixed_polys.as_ptr();
    storage.arm(None);
    storage.protocol_armed.store(true, Ordering::SeqCst);
    scalar.lock().unwrap().active = true;
    evaluations::take_clear_observations();
    let mut actual = input.evaluate_and_plan(limit).unwrap();
    assert_eq!(shared.log.lock().unwrap().events, expected.events_after);
    assert_eq!(scalar.lock().unwrap().bytes, expected.bytes);
    assert_eq!(shared.log.lock().unwrap().rng_calls, calls);
    let mut next = [0; 64];
    shared.rng.lock().unwrap().clone().fill_bytes(&mut next);
    assert_eq!(next, expected.rng_next);
    assert_eq!(storage.bank.lock().unwrap().live, originals);
    assert_eq!(
        actual.observed_inner().inner.inner.provider.inner.ordinal,
        cursor
    );
    assert_eq!(
        actual.observed_inner().inner.inner.pk.fixed_polys.as_ptr(),
        pk_pointer
    );
    let plan = actual.opening_plan().unwrap().to_vec();
    assert_eq!(plan.len(), expected.queries.len());
    for (index, (query, oracle)) in plan.iter().zip(&expected.queries).enumerate() {
        assert_eq!(query.source(), oracle.source);
        assert_eq!(query.point(), oracle.point);
        let inner = &actual.observed_inner().inner.inner;
        let (blind_source, blind) = match oracle.source {
            Source::Advice(column) => (OpeningBlind::Advice(column), None),
            Source::Instance(_) | Source::Fixed(_) | Source::Permutation(_) => {
                (OpeningBlind::Default, Some(C::Scalar::ONE))
            }
            Source::CopyProduct(i) => (
                OpeningBlind::CopyProduct(i),
                Some((inner.permutations[i].blind.0).0),
            ),
            Source::LookupProduct(i) => (
                OpeningBlind::LookupProduct(i),
                Some((inner.lookups[i].product.blind.0).0),
            ),
            Source::LookupInput(i) => (
                OpeningBlind::LookupInput(i),
                Some((inner.lookups[i].input.blind.0).0),
            ),
            Source::LookupTable(i) => (
                OpeningBlind::LookupTable(i),
                Some((inner.lookups[i].table.blind.0).0),
            ),
            Source::Quotient => (OpeningBlind::Quotient, Some(actual.observed_h_blind().0)),
            Source::Random => (OpeningBlind::Random, Some((inner.random.blind.0).0)),
        };
        assert_eq!(query.blind_source(), blind_source);
        assert_eq!(blind, oracle.blind);
        let mut copied = vec![C::Scalar::from(71); n];
        actual = actual
            .copy_opening_coefficients(index, &mut copied)
            .unwrap();
        assert_eq!(
            copied, oracle.coefficient,
            "query {index} source {:?}",
            oracle.source
        );
        assert_eq!(
            eval_polynomial(&copied, query.point()),
            eval_polynomial(&oracle.coefficient, oracle.point)
        );
    }
    assert_eq!(storage.bank.lock().unwrap().live, originals);
    assert_eq!(
        actual.observed_inner().inner.inner.provider.inner.ordinal,
        cursor
    );
    assert_eq!(shared.log.lock().unwrap().events, expected.events_after);
    assert_eq!(shared.log.lock().unwrap().rng_calls, calls);
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
    assert!(cleared >= n && zero);
    drop(actual);
    assert_eq!(storage.bank.lock().unwrap().live.len(), 1);
    check_sentinel(&mut other);
    drop(other);
    assert_dropped(&shared);
}

fn evaluation_matrix<C>()
where
    C: CurveAffine + crate::SerdeCurveAffine,
    C::Scalar: StoredAssignmentFieldV1
        + WithSmallOrderMulGroup<3>
        + FromUniformBytes<64>
        + crate::SerdePrimeField,
{
    evaluation_success::<C, 4, true, 4, false, 0>(4, false);
    evaluation_success::<C, 4, true, 4, true, 0>(4, false);
    evaluation_success::<C, 4, true, 4, true, 6>(4, false);
    evaluation_success::<C, 4, true, 4, true, 6>(4, true);
    evaluation_success::<C, 3, false, 0, false, 0>(4, true);
    evaluation_success::<C, 7, false, 1, true, 0>(5, true);
    evaluation_success::<C, 3, false, 0, true, 0>(9, true);
}

#[test]
fn both_pasta_stored_evaluations_match_actual_ordinary_scalars_bytes_opening_sources_coefficients_and_blinds()
 {
    evaluation_matrix::<EqAffine>();
    evaluation_matrix::<EpAffine>();
}

#[derive(Clone, Copy)]
enum EvaluationFailure {
    Budget,
    InitialDrift,
    Read(usize, Action),
    Scalar(usize, ScalarFault),
}

fn evaluation_failure_matrix<C>(scalar_failures: bool)
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
    let limit = evaluations::scratch_bytes::<C, InverseProvider<C>, Q, M>(&pk).unwrap();
    let values = values::<C, I>(false);
    let instance = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let shared = Shared::<C>::new();
    let storage = Controls::new();
    let scalar = Arc::new(Mutex::new(ScalarLog::default()));
    let input = evaluation_input!(&params, pk.clone(), &instance, &shared, &storage, &scalar);
    storage.arm(None);
    scalar.lock().unwrap().active = true;
    storage.protocol_armed.store(true, Ordering::SeqCst);
    let actual = input.evaluate_and_plan(limit).unwrap();
    let reads = storage.bank.lock().unwrap().events.len();
    let writes = scalar.lock().unwrap().calls;
    assert!(reads > 3 && writes > 3);
    drop(actual);
    assert_dropped(&shared);
    let mut failures = Vec::new();
    if scalar_failures {
        for index in [0, writes / 2, writes - 1] {
            failures.extend([
                EvaluationFailure::Scalar(index, ScalarFault::Error),
                EvaluationFailure::Scalar(index, ScalarFault::Panic),
            ]);
        }
        // Rebind the victim to a genuine unused quotient receipt in each fresh owner.
        failures.push(EvaluationFailure::Scalar(writes - 1, ScalarFault::Drift(0)));
    } else {
        failures.extend([EvaluationFailure::Budget, EvaluationFailure::InitialDrift]);
        for index in [0, reads - 1] {
            for action in [
                Action::Error,
                Action::Panic,
                Action::Short,
                Action::Long,
                Action::Encoding,
            ] {
                failures.push(EvaluationFailure::Read(index, action));
            }
        }
        failures.push(EvaluationFailure::Read(reads - 1, Action::Drift(0)));
    }
    for failure in failures {
        let shared = Shared::<C>::new();
        let storage = Controls::new();
        let scalar = Arc::new(Mutex::new(ScalarLog::default()));
        let mut input =
            evaluation_input!(&params, pk.clone(), &instance, &shared, &storage, &scalar);
        let victim = input.inner.pieces.last().unwrap().layout.ordinal();
        let mut other = sentinel(&mut input.inner.inner.provider, 4);
        let before = shared.log.lock().unwrap().events.clone();
        let calls = shared.log.lock().unwrap().rng_calls;
        storage.arm(None);
        storage.protocol_armed.store(true, Ordering::SeqCst);
        scalar.lock().unwrap().active = true;
        let mut supplied_limit = limit;
        let mut panics = false;
        match failure {
            EvaluationFailure::Budget => supplied_limit -= 1,
            EvaluationFailure::InitialDrift => {
                let layout = input.inner.pieces.last().unwrap().layout;
                storage.after(Some(Action::Drift(victim)), layout);
            }
            EvaluationFailure::Read(index, mut action) => {
                if matches!(action, Action::Drift(_)) {
                    action = Action::Drift(victim);
                }
                panics = action == Action::Panic;
                storage.bank.lock().unwrap().fault = Some((index, action));
            }
            EvaluationFailure::Scalar(index, mut action) => {
                if matches!(action, ScalarFault::Drift(_)) {
                    action = ScalarFault::Drift(victim);
                }
                panics = matches!(action, ScalarFault::Panic);
                scalar.lock().unwrap().fault = Some((index, action));
            }
        }
        evaluations::take_clear_observations();
        drain_blinds();
        let result = catch_unwind(AssertUnwindSafe(|| input.evaluate_and_plan(supplied_limit)));
        if panics {
            assert!(result.is_err());
        } else {
            assert!(matches!(&result, Ok(Err(_))));
            if matches!(failure, EvaluationFailure::Scalar(_, ScalarFault::Error)) {
                assert!(matches!(&result, Ok(Err(StoredLookupErrorV1::Transcript))));
            }
        }
        drop(result);
        assert_eq!(shared.log.lock().unwrap().rng_calls, calls);
        assert_eq!(storage.bank.lock().unwrap().live.len(), 1);
        let log = scalar.lock().unwrap();
        match failure {
            EvaluationFailure::Scalar(index, _) => {
                assert_eq!(log.calls, index + 1);
                assert!(log.fault.is_none());
            }
            EvaluationFailure::Budget | EvaluationFailure::InitialDrift => {
                assert_eq!(log.calls, 0);
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
            EvaluationFailure::Read(_, _) => assert!(storage.bank.lock().unwrap().fault.is_none()),
        }
        drop(log);
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
        if !matches!(
            failure,
            EvaluationFailure::Budget | EvaluationFailure::InitialDrift
        ) {
            assert!(cleared > 0);
        }
        let (blinds, zero) = drain_blinds();
        assert!(blinds > 0 && zero);
        check_sentinel(&mut other);
        drop(other);
        assert_dropped(&shared);
    }
}

#[test]
fn both_pasta_scalar_write_errors_unwinds_and_last_callback_drift_destroy_original_owners() {
    evaluation_failure_matrix::<EqAffine>(true);
    evaluation_failure_matrix::<EpAffine>(true);
}

#[test]
fn both_pasta_evaluation_budget_and_source_failures_precede_or_stop_effects_and_clear_owned_scratch()
 {
    evaluation_failure_matrix::<EqAffine>(false);
    evaluation_failure_matrix::<EpAffine>(false);
}

#[derive(Clone, Copy)]
enum CopyFailure {
    Index,
    Short,
    Long,
    Drift,
    Read(Action),
}
fn copy_failure_matrix<C>()
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
    let instance = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    for failure in [
        CopyFailure::Index,
        CopyFailure::Short,
        CopyFailure::Long,
        CopyFailure::Drift,
        CopyFailure::Read(Action::Error),
        CopyFailure::Read(Action::Panic),
        CopyFailure::Read(Action::Short),
        CopyFailure::Read(Action::Encoding),
        CopyFailure::Read(Action::Drift(0)),
    ] {
        let shared = Shared::<C>::new();
        let storage = Controls::new();
        let scalar = Arc::new(Mutex::new(ScalarLog::default()));
        let mut input =
            evaluation_input!(&params, pk.clone(), &instance, &shared, &storage, &scalar);
        let victim = input.inner.inner.random.coefficient.layout;
        let mut other = sentinel(&mut input.inner.inner.provider, 4);
        let actual = input.evaluate_and_plan(1 << 26).unwrap();
        let plan = actual.opening_plan().unwrap();
        let index = if matches!(failure, CopyFailure::Index) {
            plan.len()
        } else {
            plan.iter()
                .position(|query| query.source() == Source::Quotient)
                .unwrap()
        };
        let mut output = vec![
            C::Scalar::from(87);
            match failure {
                CopyFailure::Short => 15,
                CopyFailure::Long => 17,
                _ => 16,
            }
        ];
        let calls = shared.log.lock().unwrap().rng_calls;
        let before = shared.log.lock().unwrap().events.clone();
        storage.arm(None);
        storage.protocol_armed.store(true, Ordering::SeqCst);
        scalar.lock().unwrap().active = true;
        match failure {
            CopyFailure::Drift => storage.after(Some(Action::Drift(victim.ordinal())), victim),
            CopyFailure::Read(mut action) => {
                if matches!(action, Action::Drift(_)) {
                    action = Action::Drift(victim.ordinal());
                }
                // The first quotient piece has already populated the destination when the
                // second piece read fails; cleanup must erase a genuinely partial H fold.
                storage.bank.lock().unwrap().fault = Some((1, action));
            }
            _ => (),
        }
        evaluations::take_clear_observations();
        let result = catch_unwind(AssertUnwindSafe(|| {
            actual.copy_opening_coefficients(index, &mut output)
        }));
        if matches!(failure, CopyFailure::Read(Action::Panic)) {
            assert!(result.is_err());
        } else {
            assert!(matches!(&result, Ok(Err(_))));
        }
        drop(result);
        assert!(output.iter().all(|value| *value == C::Scalar::ZERO));
        assert_eq!(storage.bank.lock().unwrap().live.len(), 1);
        assert_eq!(shared.log.lock().unwrap().rng_calls, calls);
        assert_eq!(shared.log.lock().unwrap().events, before);
        assert_eq!(scalar.lock().unwrap().calls, 0);
        if matches!(failure, CopyFailure::Read(_)) {
            assert!(storage.bank.lock().unwrap().fault.is_none());
        } else {
            assert!(
                storage
                    .bank
                    .lock()
                    .unwrap()
                    .events
                    .iter()
                    .all(|event| matches!(event.kind, IoKind::DropSnapshot | IoKind::DropWriter))
            );
        }
        let (_, zero) = evaluations::take_clear_observations();
        assert!(zero);
        check_sentinel(&mut other);
        drop(other);
        assert_dropped(&shared);
    }
}

#[test]
fn both_pasta_opening_copy_rejects_invalid_requests_and_erases_partial_h_on_error_or_unwind() {
    copy_failure_matrix::<EqAffine>();
    copy_failure_matrix::<EpAffine>();
}
