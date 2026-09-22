//! Genuine-owner guarded inner IPA differential and positive whole-PLONK tests.
//!
//! The original dense prover remains the oracle. These small generic fixtures do not
//! authenticate Core constructors or establish whole-process or device resource guarantees.

use super::*;
use crate::{
    plonk::prover::stored::proof_evaluations::opening::inner_ipa as guarded, poly::ipa::commitment,
};

macro_rules! square_prepared {
    ($params:expr,$pk:expr,$instances:expr,$shared:expr,$storage:expr,$control:expr) => {
        prepare_single_phase_stored_ipa_prefix_v1::<C, _, _, _, Scripted<C>, _, Q, M>(
            $params,
            $pk,
            SquareCircuit(Producer::new($shared, 3)),
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
        .commit_quotient(1 << 26)
        .unwrap()
        .evaluate_and_plan(1 << 26)
        .unwrap()
        .prepare_ipa_opening(1 << 26)
        .unwrap()
    };
}

fn guarded_square<C, const Q: bool, const M: u64>()
where
    C: CurveAffine + crate::SerdeCurveAffine,
    C::Scalar: StoredAssignmentFieldV1
        + WithSmallOrderMulGroup<3>
        + FromUniformBytes<64>
        + crate::SerdePrimeField,
{
    let params = ParamsIPA::<C>::new(4);
    let key_shared = Shared::<C>::new();
    let circuit = SquareCircuit(Producer::new(&key_shared, 3));
    let vk = keygen_vk_custom(&params, &circuit, true).unwrap();
    let pk = keygen_pk(&params, vk.clone(), &circuit).unwrap();
    let values = [2, 4, 3].map(|value| vec![C::Scalar::from(value)]).to_vec();
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let ordinary = Shared::<C>::new();
    let mut transcript = RecordingTranscript::new(&ordinary);
    create_proof_consuming::<
        IPACommitmentScheme<C>,
        ProverIPA<'_, C, Q, M>,
        Challenge255<C>,
        _,
        _,
        _,
    >(
        &params,
        pk.clone(),
        SquareCircuit(Producer::new(&ordinary, 3)),
        &[&instances],
        Rng(Arc::clone(&ordinary)),
        &mut transcript,
    )
    .unwrap();
    let expected = transcript.inner.clone().finalize();
    assert!(verify_square::<C, Q, M>(&params, &vk, &expected, &values));
    let shared = Shared::<C>::new();
    let storage = Controls::new();
    let control = OpeningControls::new(&storage);
    let input = square_prepared!(&params, pk, &instances, &shared, &storage, &control);
    let originals = storage.bank.lock().unwrap().live.clone();
    let cursor = input
        .observed_inner()
        .observed_inner()
        .inner
        .inner
        .provider
        .inner
        .ordinal;
    let limit = guarded::scratch_bytes(&input).unwrap();
    storage.arm(None);
    control.log.lock().unwrap().active = true;
    evaluations::take_clear_observations();
    drain_blinds();
    let finished = input.finish_guarded_ipa(limit).unwrap();
    let actual = finished
        .observed_inner()
        .observed_inner()
        .inner
        .inner
        .transcript
        .inner
        .inner
        .clone()
        .finalize();
    assert_eq!(actual, expected);
    assert_eq!(
        shared.log.lock().unwrap().events,
        ordinary.log.lock().unwrap().events
    );
    let mut expected_rng = [0; 64];
    let mut actual_rng = [0; 64];
    shared
        .rng
        .lock()
        .unwrap()
        .clone()
        .fill_bytes(&mut actual_rng);
    ordinary
        .rng
        .lock()
        .unwrap()
        .clone()
        .fill_bytes(&mut expected_rng);
    assert_eq!(actual_rng, expected_rng);
    assert_eq!(storage.bank.lock().unwrap().live, originals);
    assert!(
        storage.bank.lock().unwrap().events.is_empty(),
        "inner IPA must not read or allocate stored sources"
    );
    assert_eq!(
        finished
            .observed_inner()
            .observed_inner()
            .inner
            .inner
            .provider
            .inner
            .ordinal,
        cursor
    );
    assert!(verify_square::<C, Q, M>(&params, &vk, &actual, &values));
    for column in 0..values.len() {
        let mut changed = values.clone();
        changed[column][0] += C::Scalar::ONE;
        assert!(!verify_square::<C, Q, M>(&params, &vk, &actual, &changed));
    }
    for offset in [0, actual.len() / 2, actual.len() - 1] {
        let mut changed = actual.clone();
        changed[offset] ^= 1;
        assert!(!verify_square::<C, Q, M>(&params, &vk, &changed, &values));
    }
    assert!(!verify_square::<C, Q, M>(
        &params,
        &vk,
        &actual[..actual.len() - 1],
        &values
    ));
    let (cleared, zero) = evaluations::take_clear_observations();
    assert!(cleared >= 2 * params.n() as usize && zero);
    let (blinds, zero) = drain_blinds();
    assert!(blinds > 0 && zero);
    drop(finished);
    assert!(storage.bank.lock().unwrap().live.is_empty());
    assert_dropped(&shared);
    opening::take_observations();
}

#[test]
fn both_pasta_guarded_inner_satisfiable_proofs_match_dense_bytes_rng_and_verify_all_instance_modes()
{
    guarded_square::<EqAffine, false, 0>();
    guarded_square::<EqAffine, true, 0>();
    guarded_square::<EqAffine, true, 6>();
    guarded_square::<EpAffine, false, 0>();
    guarded_square::<EpAffine, true, 0>();
    guarded_square::<EpAffine, true, 6>();
}

fn differential<C>(k: u32, script: Vec<C::Scalar>, zero_round: Option<usize>)
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
    let params = ParamsIPA::<C>::new(k);
    let pk = inverse_key::<C, DEGREE, MIXED, I>(&params);
    let values = values::<C, I>(false);
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
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
        .unwrap()
        .prepare_ipa_opening(1 << 26)
        .unwrap();
    let before = shared.log.lock().unwrap().events.clone();
    let oracle_shared = Shared::<C>::new();
    oracle_shared.log.lock().unwrap().events = before.clone();
    let oracle_controls = OpeningControls::new(&Controls::new());
    {
        let mut log = oracle_controls.log.lock().unwrap();
        log.active = true;
        log.scripted = script.clone();
    }
    let mut oracle_transcript = OpeningTranscript {
        inner: RecordingTranscript {
            inner: input
                .observed_inner()
                .observed_inner()
                .inner
                .inner
                .transcript
                .inner
                .inner
                .clone(),
            shared: Arc::clone(&oracle_shared),
        },
        controls: oracle_controls,
    };
    let mut oracle_rng = shared.rng.lock().unwrap().clone();
    let oracle_result = commitment::create_proof(
        &params,
        &mut oracle_rng,
        &mut oracle_transcript,
        input.observed_p(),
        input.observed_p_blind(),
        *input.observed_x3(),
    );
    assert_eq!(oracle_result.is_err(), zero_round.is_some());
    let originals = storage.bank.lock().unwrap().live.clone();
    let limit = guarded::scratch_bytes(&input).unwrap();
    storage.arm(None);
    {
        let mut log = control.log.lock().unwrap();
        log.active = true;
        log.scripted = script;
    }
    evaluations::take_clear_observations();
    drain_blinds();
    let result = input.finish_guarded_ipa(limit);
    assert_eq!(result.is_err(), zero_round.is_some());
    assert_eq!(
        shared.log.lock().unwrap().events,
        oracle_shared.log.lock().unwrap().events
    );
    let mut actual_next = [0; 64];
    let mut expected_next = [0; 64];
    shared
        .rng
        .lock()
        .unwrap()
        .clone()
        .fill_bytes(&mut actual_next);
    oracle_rng.fill_bytes(&mut expected_next);
    assert_eq!(actual_next, expected_next);
    if let Ok(finished) = result {
        assert_eq!(
            finished
                .observed_inner()
                .observed_inner()
                .inner
                .inner
                .transcript
                .inner
                .inner
                .clone()
                .finalize(),
            oracle_transcript.inner.inner.clone().finalize()
        );
        assert_eq!(storage.bank.lock().unwrap().live, originals);
        assert!(storage.bank.lock().unwrap().events.is_empty());
        drop(finished);
    } else {
        let log = control.log.lock().unwrap();
        assert_eq!(
            log.events
                .iter()
                .filter(|boundary| **boundary == Boundary::Challenge)
                .count(),
            3 + zero_round.unwrap()
        );
        assert!(!log.events.contains(&Boundary::Scalar));
        assert_eq!(log.events.last(), Some(&Boundary::Challenge));
    }
    let (cleared, zero) = evaluations::take_clear_observations();
    assert!(cleared >= 2 * params.n() as usize && zero);
    let (blinds, zero) = drain_blinds();
    assert!(blinds > 0 && zero);
    assert_eq!(storage.bank.lock().unwrap().live.len(), 1);
    check_sentinel(&mut other);
    drop(other);
    assert_dropped(&shared);
    opening::take_observations();
}

fn differential_matrix<C>()
where
    C: CurveAffine + crate::SerdeCurveAffine,
    C::Scalar: StoredAssignmentFieldV1
        + WithSmallOrderMulGroup<3>
        + FromUniformBytes<64>
        + crate::SerdePrimeField,
{
    for xi in [
        C::Scalar::ZERO,
        C::Scalar::ONE,
        -C::Scalar::ONE,
        C::Scalar::from(17),
    ] {
        for z in [C::Scalar::ZERO, C::Scalar::from(31)] {
            differential::<C>(
                4,
                vec![
                    xi,
                    z,
                    C::Scalar::ONE,
                    -C::Scalar::ONE,
                    C::Scalar::from(19),
                    C::Scalar::from(23),
                ],
                None,
            );
        }
    }
    differential::<C>(8, Vec::new(), None);
    differential::<C>(9, Vec::new(), None);
    for round in 0..4 {
        let mut script = vec![C::Scalar::ONE; 6];
        script[2 + round] = C::Scalar::ZERO;
        differential::<C>(4, script, Some(round));
    }
}
#[test]
fn both_pasta_guarded_inner_matches_original_challenge_edges_and_rejects_each_zero_round() {
    differential_matrix::<EqAffine>();
    differential_matrix::<EpAffine>();
}

#[derive(Clone, Copy)]
enum InnerFailure {
    Budget(bool),
    InitialDrift,
    Protocol(usize, OpeningFault),
}
fn failures<C>()
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
    let input = opening_input!(&params, pk.clone(), &instances, &shared, &storage, &control)
        .prepare_ipa_opening(1 << 26)
        .unwrap();
    storage.arm(None);
    control.log.lock().unwrap().active = true;
    let actual = input.finish_guarded_ipa(1 << 26).unwrap();
    let events = control.log.lock().unwrap().events.clone();
    drop(actual);
    assert_dropped(&shared);
    let sampling = |event: Boundary| {
        matches!(
            event,
            Boundary::Rng32 | Boundary::Rng64 | Boundary::Fill | Boundary::TryFill
        )
    };
    let rng_events = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| sampling(*event).then_some(index))
        .collect::<Vec<_>>();
    let samples = 16 + 1 + 2 * 4;
    assert_eq!(rng_events.len(), 8 * samples);
    assert!(
        rng_events
            .iter()
            .all(|index| events[*index] == Boundary::Rng64)
    );
    let mut failures = vec![
        InnerFailure::Budget(true),
        InnerFailure::Budget(false),
        InnerFailure::InitialDrift,
    ];
    // One late internal callback in every complete sample exercises every original
    // S coefficient, the S blind, and each round's separate L/R blind boundary.
    for sample in 0..samples {
        let index = rng_events[sample * 8 + 6];
        failures.push(InnerFailure::Protocol(index, OpeningFault::Drift(0)));
    }
    for sample in [0, 15, 16, 17, 18, samples - 1] {
        failures.push(InnerFailure::Protocol(
            rng_events[sample * 8 + 6],
            OpeningFault::Panic,
        ));
    }
    for (index, event) in events
        .iter()
        .enumerate()
        .filter(|(_, event)| !sampling(**event))
    {
        failures.push(InnerFailure::Protocol(index, OpeningFault::Panic));
        failures.push(InnerFailure::Protocol(index, OpeningFault::Drift(0)));
        if matches!(event, Boundary::Point | Boundary::Scalar) {
            failures.push(InnerFailure::Protocol(index, OpeningFault::Error));
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
            .unwrap()
            .prepare_ipa_opening(1 << 26)
            .unwrap();
        let minimum = guarded::scratch_bytes(&input).unwrap();
        let mut limit = minimum;
        let before = shared.log.lock().unwrap().events.clone();
        let calls = shared.log.lock().unwrap().rng_calls;
        storage.arm(None);
        control.log.lock().unwrap().active = true;
        let mut panics = false;
        match failure {
            InnerFailure::Budget(zero) => limit = if zero { 0 } else { minimum - 1 },
            InnerFailure::InitialDrift => {
                storage.after(Some(Action::Drift(victim.ordinal())), victim)
            }
            InnerFailure::Protocol(index, mut fault) => {
                panics = matches!(fault, OpeningFault::Panic);
                if matches!(fault, OpeningFault::Drift(_)) {
                    fault = OpeningFault::Drift(victim.ordinal());
                }
                control.log.lock().unwrap().fault = Some((index, fault));
            }
        }
        evaluations::take_clear_observations();
        drain_blinds();
        let result = catch_unwind(AssertUnwindSafe(|| input.finish_guarded_ipa(limit)));
        if panics {
            assert!(result.is_err());
        } else {
            assert!(matches!(&result, Ok(Err(_))));
            if matches!(failure, InnerFailure::Budget(_)) {
                assert!(matches!(
                    &result,
                    Ok(Err(StoredLookupErrorV1::ScratchLimit))
                ));
            }
            if matches!(failure, InnerFailure::Protocol(_, OpeningFault::Error)) {
                assert!(matches!(&result, Ok(Err(StoredLookupErrorV1::Transcript))));
            }
        }
        drop(result);
        match failure {
            InnerFailure::Budget(_) | InnerFailure::InitialDrift => {
                assert!(control.log.lock().unwrap().events.is_empty());
                assert_eq!(shared.log.lock().unwrap().events, before);
                assert_eq!(shared.log.lock().unwrap().rng_calls, calls);
            }
            InnerFailure::Protocol(index, fault) => {
                assert!(control.log.lock().unwrap().fault.is_none());
                let end = if matches!(fault, OpeningFault::Drift(_)) && sampling(events[index]) {
                    let position = rng_events.iter().position(|event| *event == index).unwrap();
                    // Complete only this single eight-callback Field::random, not the full
                    // contiguous run of adjacent S coefficient samples.
                    rng_events[(position / 8 + 1) * 8 - 1] + 1
                } else {
                    index + 1
                };
                assert_eq!(control.log.lock().unwrap().events, events[..end]);
            }
        }
        assert_eq!(storage.bank.lock().unwrap().live.len(), 1);
        assert!(
            storage
                .bank
                .lock()
                .unwrap()
                .events
                .iter()
                .all(|event| matches!(event.kind, IoKind::DropSnapshot | IoKind::DropWriter))
        );
        let (cleared, zero) = evaluations::take_clear_observations();
        assert!(zero);
        assert!(
            cleared >= params.n() as usize,
            "consumed incoming P must clear even on preflight rejection"
        );
        let (blinds, zero) = drain_blinds();
        assert!(blinds > 0 && zero);
        check_sentinel(&mut other);
        drop(other);
        assert_dropped(&shared);
        opening::take_observations();
    }
}
#[test]
fn both_pasta_guarded_inner_budget_protocol_faults_and_atomic_sample_drift_destroy_owners() {
    failures::<EqAffine>();
    failures::<EpAffine>();
}
