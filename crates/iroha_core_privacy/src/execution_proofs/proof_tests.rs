//! Qualification tests for the canonical stock adapter and its native engine.

use super::*;

fn admission(players: u8) -> GameAdmissionBodyV1 {
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{account::AccountId, game::GameAdmissionParticipantV1};
    GameAdmissionBodyV1 {
        version: 1,
        participants: (0..players)
            .map(|slot| {
                let wallet = KeyPair::try_from_seed(vec![slot + 1; 32], Algorithm::Ed25519)
                    .expect("deterministic fixture wallet");
                let input = KeyPair::try_from_seed(vec![slot + 65; 32], Algorithm::Ed25519)
                    .expect("deterministic fixture input key");
                GameAdmissionParticipantV1 {
                    account: AccountId::new(wallet.public_key().clone()),
                    input_key: input.public_key().clone(),
                    application_data: vec![slot % RACE_SKIN_COUNT_V1],
                }
            })
            .collect(),
        wagers: vec![],
        resources: vec![],
    }
}

fn late_finisher_forfeit_request(all_forfeit: bool) -> RaceProverRequestV1 {
    let mut request = request(2);
    let mut state = initial_race_state_v1(request.replay.track, 2).unwrap();
    let mut frames = Vec::new();
    while state.cars[0].finish_tick.is_none() || !state.tick.is_multiple_of(6) {
        let frame = RaceInputFrameV1 {
            tick: state.tick,
            controls: vec![1 | 32, 0],
        };
        crate::execution_proofs::race::step_race_v1(&mut state, &frame).unwrap();
        frames.push(frame);
    }
    request.replay.frames = frames;
    request.replay.dnf_events = vec![RaceDnfEventV1 {
        tick: state.tick,
        slots: if all_forfeit { vec![0, 1] } else { vec![0] },
    }];
    request.statement.transcript_root =
        race_transcript_root_v1(&request.statement.network_id, &request.replay);
    let payload = race_payload_from_request_v1(&request).unwrap();
    request.statement.outcome_hash = game_message_hash_v1(
        &request.statement.network_id,
        "session-outcome",
        &payload.outcome,
    );
    request
}

#[test]
fn corrected_binding_rejects_consistently_committed_forfeited_late_finisher_awards() {
    for all_forfeit in [false, true] {
        let mut request = late_finisher_forfeit_request(all_forfeit);
        let mut payload = race_payload_from_request_v1(&request).unwrap();
        assert_eq!(
            payload.outcome.winner_slots,
            if all_forfeit { vec![] } else { vec![1] }
        );
        validate_payload(&request.statement, &payload)
            .expect("real reference replay with corrected awards");
        let mut legacy =
            crate::execution_proofs::race::race_result_v1(&payload.final_state).unwrap();
        legacy.winners = vec![0];
        payload.outcome.winner_slots = legacy.winners.clone();
        payload.outcome.result = legacy.encode();
        payload.relation_inputs.result = legacy;
        request.statement.outcome_hash = game_message_hash_v1(
            &request.statement.network_id,
            "session-outcome",
            &payload.outcome,
        );
        assert!(matches!(
            validate_payload(&request.statement, &payload),
            Err(ExecutionProofErrorV1::Statement)
        ));
    }
}

#[test]
fn late_finisher_full_native_proof_verifies_and_wrong_profile_is_rejected() {
    let request = late_finisher_forfeit_request(true);
    let output = std::env::var_os("SORA_CARS_NATIVE_PROOF_OUTPUT").map(std::path::PathBuf::from);
    if let Some(output) = &output {
        std::fs::create_dir_all(output).unwrap();
        std::fs::write(output.join("request.nrt"), request.encode()).unwrap();
    }
    let ticks = request.replay.frames.len();
    let started = std::time::Instant::now();
    let proof = prove_race_v1(&request).expect("genuine corrected proof");
    eprintln!(
        "corrected stock proof: profile={} players=2 ticks={ticks} envelope_bytes={} prove_seconds={:.3}",
        proof.profile_id,
        proof.encode().len(),
        started.elapsed().as_secs_f64()
    );
    if let Some(output) = &output {
        std::fs::write(output.join("proof.nrt"), proof.encode()).unwrap();
    }
    assert!(
        verify_race_outcome_v1(&proof)
            .unwrap()
            .winner_slots
            .is_empty()
    );
    let mut wrong_profile = proof.clone();
    wrong_profile.profile_id = Hash::new(b"uncompiled-profile");
    assert!(crate::execution_proofs::verify_execution_proof_v1(&wrong_profile).is_err());
}

/// Eight cars on zero controls for all 5,400 ticks, a multiple of the six-tick batch: the timeout
/// path, where the two front-row cars share the distance award.
fn timeout_request() -> RaceProverRequestV1 {
    let mut request = request(8);
    request.replay.frames = (0..RACE_MAX_TICKS_V1)
        .map(|tick| RaceInputFrameV1 {
            tick,
            controls: vec![0; 8],
        })
        .collect();
    request.replay.dnf_events.clear();
    request.statement.transcript_root =
        race_transcript_root_v1(&request.statement.network_id, &request.replay);
    let payload =
        race_payload_from_request_v1(&request).expect("maximum-duration reference payload");
    request.statement.outcome_hash = game_message_hash_v1(
        &request.statement.network_id,
        "session-outcome",
        &payload.outcome,
    );
    request
}

/// Release gate: a genuine eight-car, 5,400-tick proof is byte-identical at 1, 2 and 16 Rayon
/// threads (engine proofs carry no masking), round-trips the complete envelope codec, stays within
/// every size bound, and verifies through both the adapter and the registry entry.
/// `IROHA_EXECUTION_PARITY_OUTPUT=<dir>` retains `request.nrt` and `proof.nrt`;
/// `IROHA_EXECUTION_PARITY_REQUEST=<file>` proves a retained request instead.
#[test]
#[ignore = "release gate: three full eight-car, 5,400-tick proofs; run with --release --include-ignored"]
fn full_duration_eight_car_proof_is_thread_deterministic_and_round_trips() {
    use std::{fs, path::PathBuf, time::Instant};
    let request =
        std::env::var_os("IROHA_EXECUTION_PARITY_REQUEST").map_or_else(timeout_request, |path| {
            let bytes = fs::read(path).expect("retained request");
            assert!(bytes.len() <= RACE_MAX_PROOF_BYTES_V1);
            let mut remaining = bytes.as_slice();
            let request = RaceProverRequestV1::decode(&mut remaining).expect("canonical request");
            assert!(remaining.is_empty());
            assert_eq!(request.encode(), bytes);
            assert_eq!(request.manifest.profile_id, race_profile_id_v1());
            request
        });
    // The full-duration claim needs the whole roster for every tick, whichever request is used.
    assert_eq!(request.replay.player_count, 8);
    assert_eq!(request.replay.frames.len(), RACE_MAX_TICKS_V1 as usize);
    let output = std::env::var_os("IROHA_EXECUTION_PARITY_OUTPUT").map(PathBuf::from);
    if let Some(output) = &output {
        fs::create_dir_all(output).expect("qualification output");
        fs::write(output.join("request.nrt"), request.encode()).expect("retained request");
    }
    let mut baseline = None;
    for width in [1, 2, 16] {
        let start = Instant::now();
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(width)
            .build()
            .expect("bounded CPU pool");
        assert_eq!(pool.current_num_threads(), width);
        let proof = pool.install(|| prove_race_v1(&request).expect("genuine native proof"));
        let bytes = proof.encode();
        eprintln!(
            "RaceV1 release gate: threads={width} players={} ticks={} envelope_bytes={} prove_and_self_verify_seconds={:.3}",
            request.replay.player_count,
            request.replay.frames.len(),
            bytes.len(),
            start.elapsed().as_secs_f64()
        );
        if let Some(baseline) = &baseline {
            assert_eq!(
                &bytes, baseline,
                "every proof byte, including roots, FRI folds and queries, must agree"
            );
        } else {
            baseline = Some(bytes);
        }
    }
    let bytes = baseline.expect("three proofs");
    // The complete envelope codec: canonical decoding and re-encoding of envelope, payload and wire.
    assert!(bytes.len() <= RACE_MAX_PROOF_BYTES_V1);
    let mut remaining = bytes.as_slice();
    let envelope = ExecutionProofEnvelopeV1::decode(&mut remaining).expect("canonical envelope");
    assert!(remaining.is_empty());
    assert_eq!(envelope.encode(), bytes);
    let payload = decode_payload(&envelope).expect("canonical payload");
    assert_eq!(payload.encode(), envelope.proof_bytes);
    let wire = Proof::from_bytes(&payload.stark_bytes).expect("canonical STARK wire");
    assert_eq!(wire.to_bytes(), payload.stark_bytes);
    let circuit = race_circuit::circuit(
        engine_track(payload.replay.track),
        usize::from(payload.replay.player_count),
    );
    assert!(
        payload.stark_bytes.len()
            <= iroha_execution_stark::max_proof_bytes(&circuit, &Params::standard())
                .expect("bounded wire")
    );
    assert!(payload.stark_bytes.len() <= RACE_MAX_STARK_BYTES_V1);
    eprintln!(
        "RaceV1 release gate: profile={} envelope_bytes={} stark_bytes={}",
        envelope.profile_id,
        bytes.len(),
        payload.stark_bytes.len()
    );
    // Both verification entry points accept it and agree on the distance award.
    assert_eq!(
        verify_race_outcome_v1(&envelope)
            .expect("adapter verification")
            .winner_slots,
        vec![0, 1]
    );
    assert_eq!(
        crate::execution_proofs::verify_execution_proof_v1(&envelope)
            .expect("registry verification")
            .winner_slots,
        vec![0, 1]
    );
    if let Some(output) = &output {
        fs::write(output.join("proof.nrt"), &bytes).expect("retained proof");
    }
}

fn request(players: u8) -> RaceProverRequestV1 {
    let replay = RaceReplayV1 {
        track: RaceTrackV1::NeonTokyo,
        player_count: players,
        frames: (0..6)
            .map(|tick| RaceInputFrameV1 {
                tick,
                controls: (0..players)
                    .map(|slot| 1 | if slot % 2 == 0 { 8 } else { 4 })
                    .collect(),
            })
            .collect(),
        dnf_events: vec![RaceDnfEventV1 {
            tick: 6,
            slots: (0..players).collect(),
        }],
    };
    let state = replay_race_v1(&replay).expect("reference replay");
    let network_id = NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
        iroha_data_model::block::BlockHeader,
    >::from_untyped_unchecked(Hash::new(
        b"race-proof-test-network",
    )));
    let manifest = GameManifestV1 {
        version: 1,
        application_id: Hash::new(b"test-game"),
        profile_id: race_profile_id_v1(),
        application_parameters: replay.track.encode(),
        min_participants: 2,
        max_participants: players,
        batch_ticks: 6,
        max_ticks: 5400,
        max_input_bytes: 12,
        max_participant_data_bytes: 1,
        access: iroha_data_model::game::GameAccessV1::Public,
        payout_policy: iroha_data_model::game::GamePayoutPolicyV1::EqualWinnersOrRefund,
    };
    let outcome = race_game_outcome_v1(&state).expect("outcome");
    let admission = admission(players);
    RaceProverRequestV1 {
        statement: ExecutionPublicInputsV1 {
            network_id,
            session_id: Hash::new(b"race"),
            manifest_hash: game_message_hash_v1(&network_id, "session-manifest", &manifest),
            roster_hash: game_roster_hash_v1(&network_id, &Hash::new(b"race"), &admission),
            transcript_root: race_transcript_root_v1(&network_id, &replay),
            dispute_root: Hash::new(b"history"),
            outcome_hash: game_message_hash_v1(&network_id, "session-outcome", &outcome),
        },
        manifest,
        admission,
        replay,
        checkpoint_state: None,
    }
}
#[test]
fn funding_profile_is_fail_closed_before_release_qualification() {
    assert!(!race_profile_is_qualified_v1());
}

#[test]
fn immutable_admission_is_recomputed_and_stock_entitlements_are_exact() {
    use iroha_data_model::{
        game::{GameAdmissionResourceV1, GameAdmissionWagerV1},
        game_resources::GameResourceReturnPolicyV1,
    };
    let request = request(2);
    let payload = race_payload_from_request_v1(&request).expect("canonical payload");
    validate_payload(&request.statement, &payload).expect("valid immutable admission");
    for mutation in [
        "wallet",
        "key",
        "skin",
        "missing",
        "duplicate",
        "version",
        "equipment",
        "wager-slot",
    ] {
        let mut altered = payload.clone();
        match mutation {
            "wallet" => {
                altered.admission.participants[0].account =
                    admission(8).participants[7].account.clone()
            }
            "key" => {
                altered.admission.participants[0].input_key =
                    admission(8).participants[7].input_key.clone()
            }
            "skin" => altered.admission.participants[0].application_data = vec![RACE_SKIN_COUNT_V1],
            "missing" => {
                altered.admission.participants.pop();
            }
            "duplicate" => {
                altered.admission.participants[1] = altered.admission.participants[0].clone()
            }
            "version" => altered.admission.version = 2,
            "equipment" => altered.admission.resources.push(GameAdmissionResourceV1 {
                slot: 0,
                nft_id: "kit$equipment.universal".parse().unwrap(),
                metadata_hash: Hash::new(b"complete kit metadata"),
                role_id: Hash::new(b"uncompiled kit role"),
                policy: GameResourceReturnPolicyV1::ReturnToOriginalOwnerAtTerminal,
            }),
            "wager-slot" => altered.admission.wagers.push(GameAdmissionWagerV1 {
                slot: 2,
                nft_id: "prize$equipment.universal".parse().unwrap(),
                metadata_hash: Hash::new(b"complete prize metadata"),
            }),
            _ => unreachable!(),
        }
        assert!(
            validate_payload(&request.statement, &altered).is_err(),
            "unbound {mutation}"
        );
        let mut statement = request.statement;
        statement.roster_hash = game_roster_hash_v1(
            &statement.network_id,
            &statement.session_id,
            &altered.admission,
        );
        altered.relation_inputs.roster_hash = statement.roster_hash;
        if matches!(mutation, "wallet" | "key") {
            // Different valid public claims remain structurally valid but require a new proof.
            validate_payload(&statement, &altered).expect("valid alternate public statement");
        } else {
            assert!(
                validate_payload(&statement, &altered).is_err(),
                "invalid committed {mutation}"
            );
        }
    }
    let mut wagered = payload;
    wagered.admission.wagers.push(GameAdmissionWagerV1 {
        slot: 0,
        nft_id: "prize$equipment.universal".parse().unwrap(),
        metadata_hash: Hash::new(b"complete prize metadata"),
    });
    let mut statement = request.statement;
    statement.roster_hash = game_roster_hash_v1(
        &statement.network_id,
        &statement.session_id,
        &wagered.admission,
    );
    wagered.relation_inputs.roster_hash = statement.roster_hash;
    validate_payload(&statement, &wagered).expect("explicit wager is allowed by stock relation");
    wagered.admission.wagers[0].metadata_hash = Hash::new(b"substituted metadata");
    assert!(validate_payload(&statement, &wagered).is_err());
}

/// Largest computed engine wire bound over the closed track/roster catalog (eight-car circuits).
const MAXIMUM_STARK_WIRE_BYTES: usize = 2_044_445;

fn maximum_stark_wire_bytes() -> usize {
    let params = Params::standard();
    [
        RaceTrackV1::NeonTokyo,
        RaceTrackV1::Harbor,
        RaceTrackV1::Sakura,
    ]
    .into_iter()
    .flat_map(|track| {
        (2..=8).map(move |players| race_circuit::circuit(engine_track(track), players))
    })
    .map(|circuit| {
        assert_eq!(circuit.log_rows, race_circuit::LOG_ROWS);
        params
            .check(&circuit)
            .expect("standard parameters fit every compiled circuit");
        iroha_execution_stark::max_proof_bytes(&circuit, &params).expect("bounded wire")
    })
    .max()
    .expect("nonempty catalog")
}

#[test]
fn every_compiled_circuit_fits_the_explicit_stark_cap() {
    assert_ne!(race_profile_id_v1(), race_rules_hash_v1());
    let maximum = maximum_stark_wire_bytes();
    assert_eq!(maximum, MAXIMUM_STARK_WIRE_BYTES);
    assert!(maximum <= RACE_MAX_STARK_BYTES_V1);
}

/// The profile identity commits every engine source the verifier compiles, so a changed or added
/// engine file cannot keep the RaceV1 identity.
#[test]
fn profile_commits_every_engine_source() {
    fn collect(root: &std::path::Path, dir: &std::path::Path, out: &mut Vec<(String, Vec<u8>)>) {
        for entry in std::fs::read_dir(dir).expect("engine sources") {
            let path = entry.expect("engine source").path();
            if path.is_dir() {
                collect(root, &path, out);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                let relative = path
                    .strip_prefix(root)
                    .expect("inside the engine")
                    .components()
                    .map(|part| part.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/");
                out.push((
                    format!("crates/iroha_execution_stark/src/{relative}"),
                    std::fs::read(&path).expect("engine source bytes"),
                ));
            }
        }
    }
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../iroha_execution_stark/src");
    let mut engine = Vec::new();
    collect(&root, &root, &mut engine);
    let committed: std::collections::BTreeMap<&str, &[u8]> =
        RACE_PROFILE_SOURCES_V1.iter().copied().collect();
    assert_eq!(
        committed.len(),
        RACE_PROFILE_SOURCES_V1.len(),
        "duplicate source"
    );
    assert!(engine.len() >= 15);
    for (name, bytes) in &engine {
        assert_eq!(
            committed.get(name.as_str()).copied(),
            Some(bytes.as_slice()),
            "{name} must be committed exactly"
        );
    }
    assert_eq!(
        committed
            .keys()
            .filter(|name| name.starts_with("crates/iroha_execution_stark/"))
            .count(),
        engine.len()
    );
}

/// The full-strength profile commitment frames the descriptor, the rules, the source count and
/// every (path, bytes) pair, so no edit, omission, reordering or boundary shift keeps it.
#[test]
fn profile_commitment_frames_profile_rules_and_every_source() {
    fn borrowed<'a>(sources: &'a [(&'a str, Vec<u8>)]) -> Vec<(&'a str, &'a [u8])> {
        sources
            .iter()
            .map(|(path, bytes)| (*path, bytes.as_slice()))
            .collect()
    }
    let sources = RACE_PROFILE_SOURCES_V1;
    let base = profile_commitment(PROFILE, RULES, sources);
    assert_eq!(base, race_profile_commitment_v1());
    let mut edited: Vec<(&str, Vec<u8>)> = sources
        .iter()
        .map(|(path, bytes)| (*path, bytes.to_vec()))
        .collect();
    edited.last_mut().expect("sources").1.push(b'\n');
    assert_ne!(profile_commitment(PROFILE, RULES, &borrowed(&edited)), base);
    // Moving one byte across the path/content boundary is a different commitment.
    let (path, bytes) = sources[0];
    let shifted_path = format!("{path}{}", char::from(bytes[0]));
    let mut shifted = sources.to_vec();
    shifted[0] = (shifted_path.as_str(), &bytes[1..]);
    assert_ne!(profile_commitment(PROFILE, RULES, &shifted), base);
    let mut reordered = sources.to_vec();
    reordered.swap(0, 1);
    assert_ne!(profile_commitment(PROFILE, RULES, &reordered), base);
    assert_ne!(profile_commitment(PROFILE, RULES, &sources[1..]), base);
    assert_ne!(profile_commitment(RULES, PROFILE, sources), base);
}

/// The descriptor's parameter fields are the compiled parameters, caps and target, not prose.
#[test]
fn profile_descriptor_matches_the_compiled_parameters() {
    fn field<'a>(descriptor: &'a str, name: &str) -> &'a str {
        descriptor
            .split(':')
            .find_map(|part| part.strip_prefix(name)?.strip_prefix('='))
            .unwrap_or_else(|| panic!("the descriptor names {name}"))
    }
    let descriptor = std::str::from_utf8(PROFILE).expect("ASCII descriptor");
    let params = Params::standard();
    assert_eq!(field(descriptor, "wire"), "IEB1/1");
    assert_eq!(
        field(descriptor, "trace"),
        format!("2^{}", race_circuit::LOG_ROWS)
    );
    assert_eq!(
        field(descriptor, "digest-bytes"),
        iroha_execution_stark::hash::DIGEST_BYTES.to_string()
    );
    assert_eq!(
        field(descriptor, "blowup"),
        (1u32 << params.log_blowup).to_string()
    );
    assert_eq!(field(descriptor, "queries"), params.queries.to_string());
    assert_eq!(
        field(descriptor, "fri-arity"),
        (1u32 << params.fri_log_arity).to_string()
    );
    assert_eq!(
        field(descriptor, "final-degree"),
        (1u32 << params.fri_log_final_degree).to_string()
    );
    assert_eq!(
        field(descriptor, "grinding"),
        params.grinding_bits.to_string()
    );
    assert_eq!(
        field(descriptor, "round-target"),
        iroha_execution_stark::params::PER_ROUND_TARGET_BITS.to_string()
    );
    assert_eq!(
        field(descriptor, "security-target"),
        RACE_TARGET_SECURITY_BITS_V1.to_string()
    );
    assert_eq!(
        field(descriptor, "envelope-max"),
        RACE_MAX_PROOF_BYTES_V1.to_string()
    );
    assert_eq!(
        field(descriptor, "stark-max"),
        RACE_MAX_STARK_BYTES_V1.to_string()
    );
    assert_eq!(
        compiled_race_profile_v1().target_soundness_bits,
        RACE_TARGET_SECURITY_BITS_V1
    );
}

/// Every compiled circuit meets the profile's 128-bit target with the standard parameters at the
/// Core gate, and the same check rejects a weaker query count.
#[test]
fn standard_parameters_meet_the_profile_security_target_for_every_circuit() {
    for track in [
        RaceTrackV1::NeonTokyo,
        RaceTrackV1::Harbor,
        RaceTrackV1::Sakura,
    ] {
        for players in 2..=8 {
            let circuit = race_circuit::circuit(engine_track(track), players);
            engine_params(&circuit).expect("standard parameters meet the profile target");
        }
    }
    let circuit = race_circuit::circuit(race_circuit::Track::Harbor, 8);
    let weaker = Params {
        queries: Params::standard().queries - 8,
        ..Params::standard()
    };
    weaker
        .check(&circuit)
        .expect("still a compatible configuration");
    assert_eq!(
        weaker.check_security(&circuit, f64::from(RACE_TARGET_SECURITY_BITS_V1)),
        Err(iroha_execution_stark::params::SecurityError::InsufficientSecurity)
    );
}

/// Twelve ticks with a certified checkpoint after six, then both cars removed at the final tick.
fn checkpoint_request() -> RaceProverRequestV1 {
    let mut request = request(2);
    request.replay.frames = (0..12)
        .map(|tick| RaceInputFrameV1 {
            tick,
            controls: vec![1 | 8, 1 | 4 | 32],
        })
        .collect();
    request.replay.dnf_events = vec![RaceDnfEventV1 {
        tick: 12,
        slots: vec![0, 1],
    }];
    let mut checkpoint = initial_race_state_v1(request.replay.track, 2).expect("grid");
    for frame in &request.replay.frames[..6] {
        step_race_v1(&mut checkpoint, frame).expect("reference tick");
    }
    request.checkpoint_state = Some(checkpoint);
    request.statement.transcript_root =
        race_transcript_root_v1(&request.statement.network_id, &request.replay);
    let payload = race_payload_from_request_v1(&request).expect("reference payload");
    request.statement.outcome_hash = game_message_hash_v1(
        &request.statement.network_id,
        "session-outcome",
        &payload.outcome,
    );
    request
}

/// The node's own simulation is the only witness the circuit accepts: one altered register at any
/// tick is a violated identity, not an alternative provable history.
#[test]
fn forged_node_states_cannot_be_proved() {
    let request = checkpoint_request();
    let payload = race_payload_from_request_v1(&request).expect("reference payload");
    let (circuit, statement, public) =
        engine_statement(&request.statement, &payload).expect("engine statement");
    let honest = race_register_states(&payload.replay).expect("native states");
    iroha_execution_stark::check_trace(&circuit, &statement, &public, &honest)
        .expect("the node simulation satisfies the circuit");
    // Progress, lateral position, speed and energy of car 0, then car 1's finish flag.
    for (tick, register) in [(1, 0), (4, 1), (7, 2), (9, 4), (10, 7 + 5)] {
        let mut forged = honest.clone();
        forged[tick][register] += 1;
        assert!(
            iroha_execution_stark::prove(
                &circuit,
                &Params::standard(),
                &statement,
                &public,
                &forged
            )
            .is_err(),
            "forged register {register} at tick {tick}"
        );
    }
}

/// A certified checkpoint is a proved boundary: the proof passes through that exact state, and no
/// other checkpoint state verifies or can be proved for the same replay.
#[test]
fn checkpoint_state_is_a_proved_boundary() {
    let request = checkpoint_request();
    let proof = prove_race_v1(&request).expect("genuine checkpointed proof");
    verify_race_proof_v1(&proof).expect("independent checkpointed verification");
    let mut forged = request.clone();
    forged.checkpoint_state.as_mut().expect("checkpoint").cars[0].progress_mm += 1;
    assert!(prove_race_v1(&forged).is_err());
    let mut payload = decode_payload(&proof).expect("payload");
    payload.checkpoint_state.as_mut().expect("checkpoint").cars[0].progress_mm += 1;
    validate_payload(&proof.statement, &payload).expect("structurally valid forged checkpoint");
    let mut corrupted = proof.clone();
    corrupted.proof_bytes = payload.encode();
    assert!(matches!(
        verify_race_proof_v1(&corrupted),
        Err(ExecutionProofErrorV1::Cryptography(_))
    ));
    // Removal ticks are public columns, not registers: a consistently re-committed final state
    // that hides a removal is rejected against the public schedule before any proof work.
    let mut payload = decode_payload(&proof).expect("payload");
    payload.final_state.cars[1].dnf_tick = None;
    payload.relation_inputs.result =
        race_result_v1(&payload.final_state).expect("bounded forged result");
    payload.outcome = race_game_outcome_v1(&payload.final_state).expect("forged outcome");
    let mut corrupted = proof.clone();
    corrupted.statement.outcome_hash = game_message_hash_v1(
        &corrupted.statement.network_id,
        "session-outcome",
        &payload.outcome,
    );
    validate_payload(&corrupted.statement, &payload).expect("consistently committed forgery");
    corrupted.proof_bytes = payload.encode();
    assert!(matches!(
        verify_race_proof_v1(&corrupted),
        Err(ExecutionProofErrorV1::Replay)
    ));
}

/// Final states record every removal, including those at the final tick; a checkpoint records
/// only removals strictly before its tick, matching the certified transcript prefix.
#[test]
fn removal_ticks_follow_the_public_schedule() {
    let request = checkpoint_request();
    let payload = race_payload_from_request_v1(&request).expect("reference payload");
    assert!(removals_match_states(&payload));
    let mut hidden = payload.clone();
    hidden.final_state.cars[0].dnf_tick = None;
    assert!(!removals_match_states(&hidden));
    let mut invented = payload.clone();
    invented.final_state.cars[0].dnf_tick = Some(6);
    assert!(!removals_match_states(&invented));
    let mut early = payload.clone();
    early.checkpoint_state.as_mut().expect("checkpoint").cars[0].dnf_tick = Some(12);
    assert!(!removals_match_states(&early));
    // A checkpoint at the removal tick itself still predates that removal.
    let mut at_removal = payload.clone();
    let mut state = payload.final_state.clone();
    for car in &mut state.cars {
        car.dnf_tick = None;
    }
    at_removal.checkpoint_state = Some(state.clone());
    assert!(removals_match_states(&at_removal));
    state.cars[1].dnf_tick = Some(12);
    at_removal.checkpoint_state = Some(state);
    assert!(!removals_match_states(&at_removal));
}

/// The statement binding carries each field's canonical bytes, length-prefixed and unhashed, so
/// it is exactly as strong as the engine transcript rather than the 255-bit ledger `Hash`.
#[test]
fn engine_binding_carries_canonical_fields_unhashed() {
    let request = checkpoint_request();
    let payload = race_payload_from_request_v1(&request).expect("reference payload");
    let binding = engine_binding(&request.statement, &payload);
    for field in [
        race_profile_commitment_v1().to_vec(),
        request.statement.encode(),
        payload.admission.encode(),
        payload.replay.encode(),
        payload.checkpoint_state.encode(),
    ] {
        let framed = [(field.len() as u64).to_le_bytes().as_slice(), &field].concat();
        assert!(binding.windows(framed.len()).any(|window| window == framed));
    }
}

#[test]
fn maximum_duration_replay_and_typed_settlement_fit_the_execution_cap() {
    #[derive(norito::NoritoSchema)]
    #[norito_schema(
        name = "iroha_core::execution_proofs::proof::tests::maximum_duration_replay_and_typed_settlement_fit_the_execution_cap::SettlementFields"
    )]
    #[derive(Encode)]
    struct SettlementFields {
        session_id: Hash,
        proof: ExecutionProofEnvelopeV1,
        outcome: GameOutcomeV1,
    }
    use iroha_data_model::game::{
        GAME_ADMISSION_MAX_ACCOUNT_ENCODED_BYTES_V1, GAME_ADMISSION_MAX_NFT_ENCODED_BYTES_V1,
    };
    use iroha_data_model::isi::game::SettleGameSessionV1;

    // This is a codec-dimension bound, deliberately not a generated proof.
    // Saturate every variable-length RaceV1 field, including both Option
    // variants and eight individually recorded removals. The resulting
    // shape need not be reachable: it bounds every reachable transcript.
    let request = request(8);
    let mut payload = race_payload_from_request_v1(&request).expect("payload");
    let maximum_crypto_bytes = maximum_stark_wire_bytes();
    assert_eq!(maximum_crypto_bytes, MAXIMUM_STARK_WIRE_BYTES);
    assert!(maximum_crypto_bytes <= RACE_MAX_STARK_BYTES_V1);
    payload.replay.frames = (0..RACE_MAX_TICKS_V1)
        .map(|tick| RaceInputFrameV1 {
            tick,
            controls: vec![RACE_CONTROL_MASK_V1; 8],
        })
        .collect();
    payload.replay.dnf_events = (0..8)
        .map(|slot| RaceDnfEventV1 {
            tick: u32::from(slot) * 6,
            slots: vec![slot],
        })
        .collect();
    payload.final_state.tick = RACE_MAX_TICKS_V1;
    for car in &mut payload.final_state.cars {
        car.finish_tick = Some(RACE_MAX_TICKS_V1);
        car.dnf_tick = Some(RACE_MAX_TICKS_V1);
    }
    payload.checkpoint_state = Some(payload.final_state.clone());
    payload.relation_inputs.result = RaceResultV1 {
        ticks: RACE_MAX_TICKS_V1,
        standings: (0..8)
            .map(|slot| RaceStandingV1 {
                slot,
                finish_tick: Some(RACE_MAX_TICKS_V1),
                dnf_tick: Some(RACE_MAX_TICKS_V1),
                progress_mm: 7_200_000,
            })
            .collect(),
        winners: (0..8).collect(),
    };
    payload.outcome = GameOutcomeV1 {
        terminal_tick: RACE_MAX_TICKS_V1,
        winner_slots: (0..8).collect(),
        result: payload.relation_inputs.result.encode(),
    };
    // Each native struct/sequence field needs at most a ten-byte u64 length prefix
    // (covering both compact and fixed-length framing). Accounts and NFT IDs have
    // independently enforced encoded bounds; their display lengths are not wire bounds.
    // Ed25519 keys have fixed codec geometry. Stock admission allows one cosmetic
    // byte and one wager per car, and no equipment. This conservatively covers
    // eight maximal controllers and eight maximal NFT identifiers without inventing
    // an invalid account or omitting public admission from the envelope calculation.
    let framed = |bytes: usize| bytes + 10;
    let participant_bound = framed(GAME_ADMISSION_MAX_ACCOUNT_ENCODED_BYTES_V1)
        + framed(payload.admission.participants[0].input_key.encode().len())
        + framed(vec![0_u8].encode().len());
    let wager_bound = framed(0_u8.encode().len())
        + framed(GAME_ADMISSION_MAX_NFT_ENCODED_BYTES_V1)
        + framed(Hash::new(b"bounded hash").encode().len());
    let admission_bound = framed(1_u16.encode().len())
        + framed(8 + 8 * framed(participant_bound))
        + framed(8 + 8 * framed(wager_bound))
        + framed(8);
    assert!(payload.admission.encode().len() <= admission_bound);
    // Account for growth of both the payload's admission-field prefix and its
    // enclosing byte-vector lengths as the bounded body replaces the fixture body.
    let admission_allowance = admission_bound - payload.admission.encode().len() + 40;
    payload.stark_bytes = vec![0; maximum_crypto_bytes];
    let proof = ExecutionProofEnvelopeV1 {
        version: 1,
        profile_id: race_profile_id_v1(),
        statement: request.statement,
        proof_bytes: payload.encode(),
    };
    // PublicKey serializes a compact tag + bounded byte payload. Reserve
    // the feature-independent largest supported key and 64 bytes for its
    // enum/field framing; this replaces the smaller Public access variant.
    let invitation_allowance = vec![0_u8; iroha_crypto::MAX_PUBLIC_KEY_PAYLOAD_BYTES + 1]
        .encode()
        .len()
        + 64;
    let envelope_bound = proof.encode().len() + invitation_allowance + admission_allowance;
    // Build canonical field bytes without depending on the instruction's constructor API,
    // then decode and re-encode the real registered native ISI.

    let fields = SettlementFields {
        session_id: request.statement.session_id,
        proof,
        outcome: payload.outcome,
    }
    .encode();
    let mut encoded_fields = fields.as_slice();
    let settlement =
        SettleGameSessionV1::decode(&mut encoded_fields).expect("canonical typed settlement shape");
    assert!(encoded_fields.is_empty());
    assert_eq!(settlement.encode(), fields);
    let settlement_bound = settlement.encode().len() + invitation_allowance + admission_allowance;
    eprintln!(
        "RaceV1 codec bounds: crypto={maximum_crypto_bytes} envelope={envelope_bound} typed_settlement={settlement_bound} invitation_allowance={invitation_allowance} admission_bound={admission_bound}"
    );
    assert!(envelope_bound <= RACE_MAX_PROOF_BYTES_V1);
    assert!(settlement_bound <= EXECUTION_PROOF_MAX_ENVELOPE_BYTES_V1);
    assert!(!compiled_race_profile_v1().qualified);
}

#[test]
fn proof_admission_counts_envelope_framing_and_separate_stark_cap() {
    let request = request(2);
    let mut proof = ExecutionProofEnvelopeV1 {
        version: 1,
        profile_id: race_profile_id_v1(),
        statement: request.statement,
        proof_bytes: vec![0; RACE_MAX_PROOF_BYTES_V1],
    };
    assert!(proof.encode().len() > RACE_MAX_PROOF_BYTES_V1);
    assert!(matches!(
        decode_payload(&proof),
        Err(ExecutionProofErrorV1::Envelope)
    ));
    let mut payload = race_payload_from_request_v1(&request).expect("payload");
    payload.stark_bytes = vec![0; RACE_MAX_STARK_BYTES_V1 + 1];
    proof.proof_bytes = payload.encode();
    assert!(proof.encode().len() < RACE_MAX_PROOF_BYTES_V1);
    assert!(matches!(
        decode_payload(&proof),
        Err(ExecutionProofErrorV1::Envelope)
    ));
    payload.stark_bytes.truncate(RACE_MAX_STARK_BYTES_V1);
    proof.proof_bytes = payload.encode();
    // Decoding a bounded shape is not cryptographic acceptance.
    assert!(decode_payload(&proof).is_ok());
    assert!(!race_profile_is_qualified_v1());
}

#[test]
fn timeout_outcome_binding_rejects_refund_and_false_distance_winner() {
    let mut request = timeout_request();
    let mut payload =
        race_payload_from_request_v1(&request).expect("maximum-duration reference payload");
    assert_eq!(payload.outcome.winner_slots, vec![0, 1]);
    validate_payload(&request.statement, &payload).expect("distance winner binding");
    for winners in [vec![], vec![7], vec![0]] {
        payload.relation_inputs.result.winners = winners.clone();
        payload.outcome.winner_slots = winners;
        payload.outcome.result = payload.relation_inputs.result.encode();
        request.statement.outcome_hash = game_message_hash_v1(
            &request.statement.network_id,
            "session-outcome",
            &payload.outcome,
        );
        assert!(matches!(
            validate_payload(&request.statement, &payload),
            Err(ExecutionProofErrorV1::Statement)
        ));
    }
}

#[test]
fn real_native_proof_roundtrip_and_statement_adversaries() {
    let start = std::time::Instant::now();
    let proof = prove_race_v1(&request(2)).expect("genuine native execution proof");
    eprintln!(
        "race proof bytes={} proving_seconds={:.3}",
        proof.proof_bytes.len(),
        start.elapsed().as_secs_f64()
    );
    verify_race_proof_v1(&proof).expect("verify independent envelope");
    // Keep each forged public statement internally consistent. Rejection must
    // therefore come from the actual STARK transcript/opening binding, rather
    // than merely mismatched duplicate metadata in the envelope and payload.
    for mutation in [
        "network",
        "session",
        "roster",
        "dispute",
        "application",
        "checkpoint",
    ] {
        let mut corrupted = proof.clone();
        let mut payload = decode_payload(&proof).expect("canonical payload");
        match mutation {
            "network" => {
                corrupted.statement.network_id =
                    NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
                        iroha_data_model::block::BlockHeader,
                    >::from_untyped_unchecked(
                        Hash::new(b"a different finalized genesis")
                    ));
                payload.relation_inputs.network_id = corrupted.statement.network_id;
                corrupted.statement.transcript_root =
                    race_transcript_root_v1(&corrupted.statement.network_id, &payload.replay);
                payload.relation_inputs.transcript_root = corrupted.statement.transcript_root;
            }
            "session" => {
                corrupted.statement.session_id = Hash::new(b"another game session");
                payload.relation_inputs.race_id = corrupted.statement.session_id;
            }
            "roster" => {
                payload.admission.participants[0].input_key =
                    admission(8).participants[7].input_key.clone();
            }
            "dispute" => {
                corrupted.statement.dispute_root =
                    Hash::new(b"other checkpoint epoch and forced inputs");
                payload.relation_inputs.dispute_root = corrupted.statement.dispute_root;
            }
            "application" => payload.manifest.application_id = Hash::new(b"other game application"),
            "checkpoint" => {
                payload.checkpoint_state = Some(
                    initial_race_state_v1(payload.replay.track, payload.replay.player_count)
                        .expect("valid alternate checkpoint"),
                );
            }
            _ => unreachable!(),
        }
        corrupted.statement.manifest_hash = game_message_hash_v1(
            &corrupted.statement.network_id,
            "session-manifest",
            &payload.manifest,
        );
        corrupted.statement.outcome_hash = game_message_hash_v1(
            &corrupted.statement.network_id,
            "session-outcome",
            &payload.outcome,
        );
        corrupted.statement.roster_hash = game_roster_hash_v1(
            &corrupted.statement.network_id,
            &corrupted.statement.session_id,
            &payload.admission,
        );
        payload.relation_inputs.roster_hash = corrupted.statement.roster_hash;
        validate_payload(&corrupted.statement, &payload)
            .expect("internally consistent forged statement");
        corrupted.proof_bytes = payload.encode();
        assert!(
            verify_race_proof_v1(&corrupted).is_err(),
            "unbound {mutation}"
        );
    }
    let mut corrupted = proof.clone();
    corrupted.proof_bytes.push(0);
    assert!(verify_race_proof_v1(&corrupted).is_err());
    let mut corrupted = proof.clone();
    corrupted.statement.session_id = Hash::new(b"foreign race");
    let mut foreign_payload = decode_payload(&proof).expect("payload");
    foreign_payload.relation_inputs.race_id = corrupted.statement.session_id;
    corrupted.proof_bytes = foreign_payload.encode();
    assert!(verify_race_proof_v1(&corrupted).is_err());
    let mut payload = decode_payload(&proof).expect("payload");
    payload.replay.frames[0].controls[0] ^= 32;
    let mut corrupted = proof.clone();
    corrupted.statement.transcript_root =
        race_transcript_root_v1(&corrupted.statement.network_id, &payload.replay);
    payload.relation_inputs.transcript_root = corrupted.statement.transcript_root;
    corrupted.proof_bytes = payload.encode();
    assert!(verify_race_proof_v1(&corrupted).is_err());
    let mut payload = decode_payload(&proof).expect("payload");
    payload.final_state.cars[0].progress_mm += 1;
    let mut corrupted = proof.clone();
    payload.relation_inputs.result =
        race_result_v1(&payload.final_state).expect("bounded forged result");
    payload.outcome = race_game_outcome_v1(&payload.final_state).expect("forged outcome");
    corrupted.statement.outcome_hash = game_message_hash_v1(
        &corrupted.statement.network_id,
        "session-outcome",
        &payload.outcome,
    );
    corrupted.proof_bytes = payload.encode();
    assert!(verify_race_proof_v1(&corrupted).is_err());
    let mut corrupted = proof.clone();
    let middle = corrupted.proof_bytes.len() / 2;
    corrupted.proof_bytes[middle] ^= 1;
    assert!(verify_race_proof_v1(&corrupted).is_err());
}
