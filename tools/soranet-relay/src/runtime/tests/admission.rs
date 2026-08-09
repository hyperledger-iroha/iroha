//! Admission-binding and replay-protection tests for the relay runtime.

use super::*;

fn in_memory_ticket_replays(capacity: usize) -> StdMutex<TicketReplayState> {
    let limits =
        TicketRevocationStoreLimits::new(capacity, Duration::from_secs(300)).expect("limits");
    let persisted = TicketRevocationStore::in_memory(limits).expect("replay store");
    StdMutex::new(TicketReplayState {
        persisted,
        pending: HashSet::new(),
        capacity,
    })
}

fn client_hello_frame_with_resume(resume_hash: Option<&[u8]>) -> Vec<u8> {
    let mut frame = Vec::new();
    frame.push(crate::handshake::CLIENT_HELLO_TYPE);
    frame.extend_from_slice(&32u16.to_be_bytes());
    frame.extend_from_slice(&[0xAA; 32]);
    frame.push(1);
    frame.push(1);
    frame.extend_from_slice(&[0x11; 32]);
    frame.extend_from_slice(&4u16.to_be_bytes());
    frame.extend_from_slice(&[0x22; 4]);
    frame.extend_from_slice(&2u16.to_be_bytes());
    frame.extend_from_slice(&[0x80, 0x01]);
    match resume_hash {
        Some(resume_hash) => {
            frame.push(1);
            frame.extend_from_slice(
                &u16::try_from(resume_hash.len())
                    .expect("test resume hash length fits")
                    .to_be_bytes(),
            );
            frame.extend_from_slice(resume_hash);
        }
        None => frame.push(0),
    }
    frame.resize(crate::handshake::NOISE_PADDING_BLOCK, 0);
    frame
}

#[test]
fn admission_transcript_commits_to_the_exact_client_hello() {
    let without_resume = client_hello_frame_with_resume(None);
    let with_resume = client_hello_frame_with_resume(Some(&[0x44; 32]));
    ClientHello::parse(&without_resume).expect("parse hello without resume hash");
    ClientHello::parse(&with_resume).expect("parse hello with resume hash");

    let first = pow::derive_admission_transcript(&without_resume);
    assert_eq!(
        first,
        pow::derive_admission_transcript(&without_resume),
        "the same client hello must derive the same binding"
    );
    assert_ne!(
        first,
        pow::derive_admission_transcript(&with_resume),
        "changing any client hello field must change the admission binding"
    );
}

#[test]
fn rejected_admission_never_runs_expensive_handshake() {
    let expensive_ran = std::cell::Cell::new(false);
    let result = continue_after_admission::<()>(
        Err(HandshakeError::ReplayStore("rejected".to_owned())),
        || {
            expensive_ran.set(true);
            Ok(())
        },
    );
    assert!(matches!(
        result,
        Err(HandshakeError::ReplayStore(message)) if message == "rejected"
    ));
    assert!(
        !expensive_ran.get(),
        "ML-KEM handshake work must stay behind admission"
    );
}

#[test]
fn verify_puzzle_ticket_requires_binding_and_consumes_once() {
    let params = PuzzleParameters::new(
        NonZeroU32::new(puzzle::MIN_MEMORY_KIB).expect("non-zero memory"),
        NonZeroU32::new(1).expect("non-zero iterations"),
        NonZeroU32::new(1).expect("non-zero lanes"),
        1,
        Duration::from_secs(180),
        Duration::from_secs(45),
    );
    let descriptor = vec![0xD4; 32];
    let relay_id = vec![0xC3; 32];
    let admission_transcript = [0x9Au8; 32];
    let mut rng = StdRng::from_seed([0x5Au8; 32]);
    let replays = in_memory_ticket_replays(4);

    let binding = PuzzleBinding::new(&descriptor, &relay_id, &admission_transcript);
    let ticket = puzzle::mint_ticket(&params, &binding, Duration::from_secs(120), &mut rng)
        .expect("mint transcript-bound ticket");
    verify_puzzle_ticket_binding(
        &ticket,
        &params,
        &descriptor,
        &relay_id,
        &admission_transcript,
        &replays,
    )
    .expect("ticket should verify with matching admission transcript");
    assert!(matches!(
        verify_puzzle_ticket_binding(
            &ticket,
            &params,
            &descriptor,
            &relay_id,
            &admission_transcript,
            &replays,
        ),
        Err(HandshakeError::Pow(pow::Error::Replay))
    ));

    let mismatched = [0x44u8; 32];
    let mismatched_replays = in_memory_ticket_replays(4);
    let mismatched_ticket =
        puzzle::mint_ticket(&params, &binding, Duration::from_secs(120), &mut rng)
            .expect("mint second ticket");
    let err = verify_puzzle_ticket_binding(
        &mismatched_ticket,
        &params,
        &descriptor,
        &relay_id,
        &mismatched,
        &mismatched_replays,
    )
    .expect_err("mismatched admission transcript must fail verification");
    match err {
        HandshakeError::Puzzle(puzzle::Error::InvalidSolution) => {}
        other => panic!("unexpected puzzle verification error: {other:?}"),
    }
}

#[test]
fn verify_puzzle_ticket_rejects_wrong_relay_binding() {
    let params = PuzzleParameters::new(
        NonZeroU32::new(4_096).expect("non-zero memory"),
        NonZeroU32::new(1).expect("non-zero iterations"),
        NonZeroU32::new(1).expect("non-zero lanes"),
        5,
        Duration::from_secs(120),
        Duration::from_secs(30),
    );
    let descriptor = vec![0x51; 32];
    let relay_id = vec![0x42; 32];
    let admission_transcript = [0x24u8; 32];
    let mut rng = StdRng::from_seed([0x91u8; 32]);

    let binding = PuzzleBinding::new(&descriptor, &relay_id, &admission_transcript);
    let ticket = puzzle::mint_ticket(&params, &binding, Duration::from_secs(50), &mut rng)
        .expect("mint ticket with relay binding");
    let mismatched_relay = vec![0x99; 32];
    let replays = in_memory_ticket_replays(4);

    let err = verify_puzzle_ticket_binding(
        &ticket,
        &params,
        &descriptor,
        &mismatched_relay,
        &admission_transcript,
        &replays,
    )
    .expect_err("relay mismatch must fail verification");
    match err {
        HandshakeError::Puzzle(puzzle::Error::InvalidSolution) => {}
        other => panic!("unexpected puzzle verification error: {other:?}"),
    }
}

#[test]
fn verify_pow_ticket_rejects_wrong_relay_binding() {
    let params = PowParameters::new(16, Duration::from_secs(180), Duration::from_secs(45));
    let descriptor = [0xAA; 32];
    let relay_a = [0x01; 32];
    let relay_b = [0x02; 32];
    let transcript = [0x03; 32];
    let mut rng = StdRng::from_seed([0x22; 32]);

    let binding = pow::ChallengeBinding::new(&descriptor, &relay_a, &transcript);
    let ticket = pow::mint_ticket(&params, &binding, Duration::from_secs(60), &mut rng)
        .expect("mint pow ticket");
    let replays = in_memory_ticket_replays(4);

    let err = verify_pow_ticket_binding(
        &ticket,
        &params,
        &descriptor,
        &relay_b,
        &transcript,
        &replays,
    )
    .expect_err("relay mismatch must fail verification");
    match err {
        HandshakeError::Pow(pow::Error::InvalidSolution) => {}
        other => panic!("unexpected pow verification error: {other:?}"),
    }
}

#[test]
fn verify_pow_ticket_respects_transcript_binding() {
    let params = PowParameters::new(16, Duration::from_secs(120), Duration::from_secs(30));
    let descriptor = [0x0C; 32];
    let relay_id = [0x0D; 32];
    let transcript = [0xFE; 32];
    let mut rng = StdRng::from_seed([0x33; 32]);

    let binding = pow::ChallengeBinding::new(&descriptor, &relay_id, &transcript);
    let ticket = pow::mint_ticket(&params, &binding, Duration::from_secs(40), &mut rng)
        .expect("mint pow ticket with transcript");
    let replays = in_memory_ticket_replays(4);

    let mismatched = [0xAA; 32];
    let err = verify_pow_ticket_binding(
        &ticket,
        &params,
        &descriptor,
        &relay_id,
        &mismatched,
        &replays,
    )
    .expect_err("mismatched transcript must fail verification");
    match err {
        HandshakeError::Pow(pow::Error::InvalidSolution) => {}
        other => panic!("unexpected pow verification error: {other:?}"),
    }
}

#[test]
fn relay_ticket_replay_is_rejected_after_store_reload() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("relay-ticket-replays.norito");
    let limits = TicketRevocationStoreLimits::new(4, Duration::from_secs(300)).expect("limits");
    let params = PowParameters::new(0, Duration::from_secs(180), Duration::from_secs(30));
    let descriptor = [0x35; 32];
    let relay_id = [0x46; 32];
    let transcript = [0x57; 32];
    let binding = pow::ChallengeBinding::new(&descriptor, &relay_id, &transcript);
    let mut rng = StdRng::from_seed([0x68; 32]);
    let ticket = pow::mint_ticket(&params, &binding, Duration::from_secs(60), &mut rng)
        .expect("mint ticket");

    let persisted =
        TicketRevocationStore::load(&path, limits, SystemTime::now()).expect("load store");
    let replays = StdMutex::new(TicketReplayState {
        persisted,
        pending: HashSet::new(),
        capacity: limits.max_entries,
    });
    verify_pow_ticket_binding(
        &ticket,
        &params,
        &descriptor,
        &relay_id,
        &transcript,
        &replays,
    )
    .expect("first ticket use");
    drop(replays);

    let persisted =
        TicketRevocationStore::load(&path, limits, SystemTime::now()).expect("reload store");
    let reloaded = StdMutex::new(TicketReplayState {
        persisted,
        pending: HashSet::new(),
        capacity: limits.max_entries,
    });
    assert!(matches!(
        verify_pow_ticket_binding(
            &ticket,
            &params,
            &descriptor,
            &relay_id,
            &transcript,
            &reloaded,
        ),
        Err(HandshakeError::Pow(pow::Error::Replay))
    ));
}

#[test]
fn full_replay_store_rejects_before_costly_ticket_verification() {
    let replays = in_memory_ticket_replays(1);
    let params = PowParameters::new(0, Duration::from_secs(180), Duration::from_secs(30));
    let descriptor = [0x11; 32];
    let relay_id = [0x22; 32];
    let transcript = [0x33; 32];
    let binding = pow::ChallengeBinding::new(&descriptor, &relay_id, &transcript);
    let mut rng = StdRng::from_seed([0x44; 32]);
    let first =
        pow::mint_ticket(&params, &binding, Duration::from_secs(60), &mut rng).expect("mint first");
    let second = pow::mint_ticket(&params, &binding, Duration::from_secs(60), &mut rng)
        .expect("mint second");

    verify_and_consume_ticket(&first, &replays, || Ok(())).expect("consume first");
    let costly_verify_ran = std::cell::Cell::new(false);
    let err = verify_and_consume_ticket(&second, &replays, || {
        costly_verify_ran.set(true);
        Ok(())
    })
    .expect_err("capacity must fail closed");
    assert!(matches!(err, HandshakeError::ReplayStore(_)));
    assert!(
        !costly_verify_ran.get(),
        "capacity gate must run before Argon2 or ML-KEM work"
    );
}

#[test]
fn concurrent_duplicate_ticket_is_rejected_while_first_use_is_pending() {
    let replays = Arc::new(in_memory_ticket_replays(2));
    let params = PowParameters::new(0, Duration::from_secs(180), Duration::from_secs(30));
    let descriptor = [0x71; 32];
    let relay_id = [0x72; 32];
    let transcript = [0x73; 32];
    let binding = pow::ChallengeBinding::new(&descriptor, &relay_id, &transcript);
    let mut rng = StdRng::from_seed([0x74; 32]);
    let ticket = pow::mint_ticket(&params, &binding, Duration::from_secs(60), &mut rng)
        .expect("mint ticket");
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();

    let first_replays = Arc::clone(&replays);
    let first = std::thread::spawn(move || {
        verify_and_consume_ticket(&ticket, first_replays.as_ref(), || {
            entered_tx.send(()).expect("signal pending verification");
            release_rx.recv().expect("release pending verification");
            Ok(())
        })
    });
    entered_rx.recv().expect("first verification entered");

    let second_verify_ran = std::cell::Cell::new(false);
    let duplicate = verify_and_consume_ticket(&ticket, replays.as_ref(), || {
        second_verify_ran.set(true);
        Ok(())
    })
    .expect_err("concurrent duplicate must fail");
    assert!(matches!(duplicate, HandshakeError::Pow(pow::Error::Replay)));
    assert!(
        !second_verify_ran.get(),
        "duplicate must be rejected before verification work"
    );

    release_tx.send(()).expect("release first verification");
    first
        .join()
        .expect("first verification thread")
        .expect("first ticket use succeeds");
}

#[test]
fn pow_failure_reason_labels_signature_and_absent_key_cases() {
    let signature = pow::Error::InvalidSignature;
    assert_eq!(
        pow_failure_reason(&signature),
        SoranetPowFailureReasonV1::SignatureInvalid
    );

    let malformed = pow::Error::Malformed("signed ticket payload".to_string());
    assert_eq!(
        pow_failure_reason(&malformed),
        SoranetPowFailureReasonV1::UnsupportedVersion
    );

    let overflow = pow::Error::ExpiryTimestampOverflow(u64::MAX);
    assert_eq!(
        pow_failure_reason(&overflow),
        SoranetPowFailureReasonV1::ClockError
    );
}
