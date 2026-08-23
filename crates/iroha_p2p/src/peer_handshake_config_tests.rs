// SoraNet handshake configuration regressions included from `peer`.
use super::*;
use rand::{
    RngCore, SeedableRng,
    rand_core::{TryCryptoRng, TryRngCore},
    rngs::StdRng,
};
use soranet_pq::{MlDsaSuite, generate_mldsa_keypair_from_os as generate_mldsa_keypair};
use std::{
    fmt,
    num::{NonZeroU32, NonZeroUsize},
};
use tempfile::tempdir;
fn test_admission_transcript() -> [u8; 32] {
    pow::derive_admission_transcript(b"soranet-test-client-hello")
}
fn substituted_admission_transcript() -> [u8; 32] {
    pow::derive_admission_transcript(b"soranet-test-client-hello-substituted")
}
fn minimal_puzzle_config(
    ticket_ttl: Duration,
    max_future_skew: Duration,
    min_ticket_ttl: Duration,
) -> Arc<SoranetHandshakeConfig> {
    let pow_params = PowParameters::new(1, max_future_skew, min_ticket_ttl);
    let puzzle_params = PuzzleParameters::new(
        NonZeroU32::new(puzzle::MIN_MEMORY_KIB).expect("minimum puzzle memory is non-zero"),
        NonZeroU32::new(1).expect("one Argon2 iteration is non-zero"),
        NonZeroU32::new(1).expect("one Argon2 lane is non-zero"),
        1,
        max_future_skew,
        min_ticket_ttl,
    );
    Arc::new(
        SoranetHandshakeConfig::new(
            iroha_crypto::soranet::handshake::DEFAULT_DESCRIPTOR_COMMIT.to_vec(),
            iroha_crypto::soranet::handshake::DEFAULT_CLIENT_CAPABILITIES.to_vec(),
            iroha_crypto::soranet::handshake::DEFAULT_RELAY_CAPABILITIES.to_vec(),
            true,
            1,
            1,
            None,
            true,
            pow_params,
            Some(puzzle_params),
            ticket_ttl,
            None,
            None,
            None,
        )
        .expect("test SoraNet handshake config must be valid"),
    )
}
fn mint_test_admission_ticket(
    config: &SoranetHandshakeConfig,
    transcript_hash: &[u8; 32],
    seed: [u8; 32],
) -> Vec<u8> {
    config
        .mint_challenge_ticket(transcript_hash, &mut StdRng::from_seed(seed))
        .expect("test admission should mint")
        .expect("admission should be enabled")
        .frames
        .into_iter()
        .next()
        .expect("admission should produce a frame")
}
struct FailingTryRng;
struct ZeroTryRng;
struct RepeatedTryRng;
#[derive(Debug)]
struct FailingTryRngError;
impl fmt::Display for FailingTryRngError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("failing p2p ticket RNG")
    }
}
impl TryRngCore for FailingTryRng {
    type Error = FailingTryRngError;
    fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
        Err(FailingTryRngError)
    }
    fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
        Err(FailingTryRngError)
    }
    fn try_fill_bytes(&mut self, _dst: &mut [u8]) -> Result<(), Self::Error> {
        Err(FailingTryRngError)
    }
}
impl TryCryptoRng for FailingTryRng {}
impl TryRngCore for ZeroTryRng {
    type Error = std::convert::Infallible;
    fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
        Ok(0)
    }
    fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
        Ok(0)
    }
    fn try_fill_bytes(&mut self, dst: &mut [u8]) -> Result<(), Self::Error> {
        dst.fill(0);
        Ok(())
    }
}
impl TryCryptoRng for ZeroTryRng {}
impl TryRngCore for RepeatedTryRng {
    type Error = std::convert::Infallible;
    fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
        Ok(u32::from_le_bytes([0xA5; 4]))
    }
    fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
        Ok(u64::from_le_bytes([0xA5; 8]))
    }
    fn try_fill_bytes(&mut self, dst: &mut [u8]) -> Result<(), Self::Error> {
        dst.fill(0xA5);
        Ok(())
    }
}
impl TryCryptoRng for RepeatedTryRng {}
#[test]
fn soranet_handshake_rng_reads_os_entropy() {
    let mut rng = soranet_handshake_rng().expect("OS RNG should seed SoraNet handshake RNG");
    let mut bytes = [0u8; 32];
    rng.fill_bytes(&mut bytes);
}
#[test]
fn soranet_transport_delegation_challenge_rng_failure_is_fail_closed() {
    let error = generate_soranet_transport_delegation_challenge(&mut FailingTryRng)
        .expect_err("challenge entropy failure must stop the handshake");
    assert!(matches!(
        error,
        Error::HandshakeSoranet(message)
            if message == "SoraNet delegation challenge RNG failed: failing p2p ticket RNG"
    ));
}
#[test]
fn soranet_transport_delegation_challenge_rejects_all_zero_entropy() {
    let error = generate_soranet_transport_delegation_challenge(&mut ZeroTryRng)
        .expect_err("an all-zero challenge must stop the handshake");
    assert!(matches!(
        error,
        Error::HandshakeSoranet(message)
            if message == "SoraNet delegation challenge RNG returned an all-zero value"
    ));

    let error = generate_soranet_transport_delegation_challenge(&mut RepeatedTryRng)
        .expect_err("an all-identical-byte challenge must stop the handshake");
    assert!(matches!(
        error,
        Error::HandshakeSoranet(message)
            if message == "SoraNet delegation challenge RNG returned all-identical-byte material"
    ));
}
#[test]
fn rejects_invalid_kem_and_signature_ids() {
    let params = PowParameters::new(0, Duration::from_secs(300), Duration::from_secs(30));
    let kem_error = SoranetHandshakeConfig::new(
        iroha_crypto::soranet::handshake::DEFAULT_DESCRIPTOR_COMMIT.to_vec(),
        iroha_crypto::soranet::handshake::DEFAULT_CLIENT_CAPABILITIES.to_vec(),
        iroha_crypto::soranet::handshake::DEFAULT_RELAY_CAPABILITIES.to_vec(),
        true,
        42,
        1,
        None,
        false,
        params,
        None,
        Duration::from_secs(60),
        None,
        None,
        None,
    )
    .expect_err("unsupported KEM identifiers must fail closed");
    assert!(matches!(
        kem_error,
        Error::HandshakeSoranet(message)
            if message == "unsupported SoraNet ML-KEM identifier 42"
    ));
    let signature_error = SoranetHandshakeConfig::new(
        iroha_crypto::soranet::handshake::DEFAULT_DESCRIPTOR_COMMIT.to_vec(),
        iroha_crypto::soranet::handshake::DEFAULT_CLIENT_CAPABILITIES.to_vec(),
        iroha_crypto::soranet::handshake::DEFAULT_RELAY_CAPABILITIES.to_vec(),
        true,
        1,
        99,
        None,
        false,
        params,
        None,
        Duration::from_secs(60),
        None,
        None,
        None,
    )
    .expect_err("unsupported signature identifiers must fail closed");
    assert!(matches!(
        signature_error,
        Error::HandshakeSoranet(message)
            if message == "unsupported SoraNet signature identifier 99"
    ));

    SoranetHandshakeConfig::new(
        iroha_crypto::soranet::handshake::DEFAULT_DESCRIPTOR_COMMIT.to_vec(),
        iroha_crypto::soranet::handshake::DEFAULT_CLIENT_CAPABILITIES.to_vec(),
        iroha_crypto::soranet::handshake::DEFAULT_RELAY_CAPABILITIES.to_vec(),
        true,
        MlKemSuite::MlKem512.kem_id(),
        1,
        None,
        false,
        params,
        None,
        Duration::from_secs(60),
        None,
        None,
        None,
    )
    .expect("every ML-KEM suite in the first-release registry must be accepted");
}
#[test]
fn rejects_noncanonical_transcript_fields_and_capability_vectors_at_construction() {
    let params = PowParameters::new(0, Duration::from_secs(300), Duration::from_secs(30));
    let build =
        |descriptor_commit: Vec<u8>, client_capabilities: Vec<u8>, resume_hash: Option<Vec<u8>>| {
            SoranetHandshakeConfig::new(
                descriptor_commit,
                client_capabilities,
                iroha_crypto::soranet::handshake::DEFAULT_RELAY_CAPABILITIES.to_vec(),
                true,
                1,
                1,
                resume_hash,
                false,
                params,
                None,
                Duration::from_secs(60),
                None,
                None,
                None,
            )
        };
    for (error, expected) in [
        (
            build(
                vec![0; iroha_crypto::Hash::LENGTH - 1],
                iroha_crypto::soranet::handshake::DEFAULT_CLIENT_CAPABILITIES.to_vec(),
                None,
            )
            .expect_err("short descriptor commitment must fail"),
            "descriptor commitment must be 32 bytes",
        ),
        (
            build(
                iroha_crypto::soranet::handshake::DEFAULT_DESCRIPTOR_COMMIT.to_vec(),
                iroha_crypto::soranet::handshake::DEFAULT_CLIENT_CAPABILITIES.to_vec(),
                Some(vec![0; iroha_crypto::Hash::LENGTH - 1]),
            )
            .expect_err("short resume hash must fail"),
            "resume hash must be 32 bytes",
        ),
        (
            build(
                iroha_crypto::soranet::handshake::DEFAULT_DESCRIPTOR_COMMIT.to_vec(),
                vec![0x01],
                None,
            )
            .expect_err("malformed capabilities must fail"),
            "invalid SoraNet client capability vector",
        ),
        (
            build(
                iroha_crypto::soranet::handshake::DEFAULT_DESCRIPTOR_COMMIT.to_vec(),
                vec![
                    0x01, 0x02, 0x00, 0x02, 0x01, 0x01, // signature
                    0x01, 0x01, 0x00, 0x02, 0x01, 0x01, // KEM
                ],
                None,
            )
            .expect_err("decreasing client capability types must fail at construction"),
            "nondecreasing order",
        ),
        (
            build(
                iroha_crypto::soranet::handshake::DEFAULT_DESCRIPTOR_COMMIT.to_vec(),
                vec![
                    0;
                    iroha_crypto::soranet::handshake::MAX_CAPABILITY_VECTOR_LEN + 1
                ],
                None,
            )
            .expect_err("oversized client capability vector must fail at the shared bound"),
            "first-release maximum is 4096 bytes",
        ),
    ] {
        assert!(
            matches!(error, Error::HandshakeSoranet(message) if message.contains(expected)),
            "unexpected error for {expected}"
        );
    }
}
#[test]
fn admission_transcript_binds_resumption_presence_and_value() {
    let absent = RuntimeParams::soranet_defaults();
    let resume_a = [0xA1; 32];
    let resume_b = [0xB2; 32];
    let mut present_a = absent.clone();
    present_a.resume_hash = Some(&resume_a);
    let mut present_b = absent.clone();
    present_b.resume_hash = Some(&resume_b);
    let seed = [0x73; 32];
    let (hello_absent, _) =
        build_client_hello(&absent, &mut StdRng::from_seed(seed)).expect("client hello");
    let (hello_a, _) =
        build_client_hello(&present_a, &mut StdRng::from_seed(seed)).expect("resumed hello a");
    let (hello_b, _) =
        build_client_hello(&present_b, &mut StdRng::from_seed(seed)).expect("resumed hello b");
    assert_ne!(hello_absent, hello_a);
    assert_ne!(hello_absent, hello_b);
    assert_ne!(hello_a, hello_b);
    assert!(
        hello_a
            .windows(resume_a.len())
            .any(|window| window == resume_a.as_slice())
    );
    assert!(
        hello_b
            .windows(resume_b.len())
            .any(|window| window == resume_b.as_slice())
    );
    let transcript_absent = pow::derive_admission_transcript(&hello_absent);
    let transcript_a = pow::derive_admission_transcript(&hello_a);
    let transcript_b = pow::derive_admission_transcript(&hello_b);
    assert_ne!(transcript_absent, transcript_a);
    assert_ne!(transcript_absent, transcript_b);
    assert_ne!(transcript_a, transcript_b);
}
#[test]
fn puzzle_ticket_mints_and_verifies() {
    let pow_params = PowParameters::new(5, Duration::from_secs(900), Duration::from_secs(120));
    let puzzle_params = puzzle::Parameters::new(
        NonZeroU32::new(64 * 1024).expect("memory"),
        NonZeroU32::new(2).expect("time"),
        NonZeroU32::new(1).expect("lanes"),
        2,
        Duration::from_secs(900),
        Duration::from_secs(120),
    );
    let config = SoranetHandshakeConfig::new(
        iroha_crypto::soranet::handshake::DEFAULT_DESCRIPTOR_COMMIT.to_vec(),
        iroha_crypto::soranet::handshake::DEFAULT_CLIENT_CAPABILITIES.to_vec(),
        iroha_crypto::soranet::handshake::DEFAULT_RELAY_CAPABILITIES.to_vec(),
        true,
        1,
        1,
        None,
        true,
        pow_params,
        Some(puzzle_params),
        Duration::from_secs(240),
        None,
        None,
        None,
    )
    .expect("test SoraNet handshake config must be valid");
    assert_eq!(config.pow_parameters().difficulty(), 5);
    assert_eq!(config.pow_ticket_ttl(), Duration::from_secs(240));
    let configured_puzzle = config
        .puzzle_parameters()
        .expect("puzzle parameters available");
    assert_eq!(configured_puzzle.memory_kib().get(), 64 * 1024);
    let admission = config
        .admission_summary()
        .expect("admission summary present");
    assert_eq!(admission.pow.difficulty(), 5);
    assert_eq!(admission.ticket_ttl, Duration::from_secs(240));
    let mut rng = StdRng::from_seed([7u8; 32]);
    let transcript = test_admission_transcript();
    let minted = config
        .mint_challenge_ticket(&transcript, &mut rng)
        .expect("mint ticket")
        .expect("ticket bytes present");
    assert_eq!(
        minted
            .admission
            .expect("admission present")
            .pow
            .difficulty(),
        puzzle_params.difficulty()
    );
    let verification = config
        .verify_challenge_ticket(&minted.frames[0], &transcript)
        .expect("verify ticket");
    assert_eq!(
        verification.expect("verification summary").pow.difficulty(),
        puzzle_params.difficulty()
    );
    let mut corrupted = minted
        .frames
        .into_iter()
        .next()
        .expect("ticket frame present");
    // Corrupt the version byte to guarantee a parse/verify failure.
    // Flipping solution bytes is probabilistic for low difficulties (it may still satisfy
    // the leading-zero predicate), so do not rely on it in tests.
    corrupted[0] ^= 0xFF;
    assert!(
        config
            .verify_challenge_ticket(&corrupted, &transcript)
            .is_err()
    );
}
#[test]
fn token_frame_emitted_when_configured() {
    let pow_params = PowParameters::new(5, Duration::from_secs(900), Duration::from_secs(120));
    let mut config = SoranetHandshakeConfig::new(
        iroha_crypto::soranet::handshake::DEFAULT_DESCRIPTOR_COMMIT.to_vec(),
        iroha_crypto::soranet::handshake::DEFAULT_CLIENT_CAPABILITIES.to_vec(),
        iroha_crypto::soranet::handshake::DEFAULT_RELAY_CAPABILITIES.to_vec(),
        true,
        1,
        1,
        None,
        true,
        pow_params,
        None,
        Duration::from_secs(240),
        None,
        None,
        None,
    )
    .expect("test SoraNet handshake config must be valid");
    let suite = MlDsaSuite::MlDsa44;
    let keypair = generate_mldsa_keypair(suite).expect("keygen");
    let issuer_fingerprint =
        iroha_crypto::soranet::token::compute_issuer_fingerprint(keypair.public_key());
    let issued_at = std::time::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    let mut token_rng = StdRng::from_seed([0x47; 32]);
    let token = AdmissionToken::mint(
        suite,
        keypair.secret_key(),
        issuer_fingerprint,
        [0x11; 32],
        test_admission_transcript(),
        issued_at,
        issued_at + Duration::from_secs(60),
        0,
        &mut token_rng,
    )
    .expect("mint admission token");
    let encoded = token.encode();
    config
        .set_admission_token(encoded.clone())
        .expect("canonical admission token");
    let config_debug = format!("{config:?}");
    assert!(config_debug.contains("[REDACTED]"));
    assert!(!config_debug.contains(&format!("{:?}", encoded)));
    let mut rng = StdRng::from_seed([0x99; 32]);
    let transcript = test_admission_transcript();
    let minted = config
        .mint_challenge_ticket(&transcript, &mut rng)
        .expect("mint token challenge")
        .expect("token frame present");
    assert!(minted.admission.is_none());
    assert_eq!(minted.frames.len(), 1);
    assert_eq!(minted.frames[0], encoded);
    let minted_debug = format!("{minted:?}");
    assert!(minted_debug.contains("[REDACTED]"));
    assert!(!minted_debug.contains(&format!("{:?}", encoded)));
}
#[test]
fn admission_token_configuration_rejects_malformed_frames() {
    let mut config = SoranetHandshakeConfig::defaults();
    let error = config
        .set_admission_token(b"SNTK\x01".to_vec())
        .expect_err("truncated admission token must fail closed");
    assert!(matches!(error, AdmissionTokenDecodeError::Truncated { .. }));
    assert!(config.admission_token.is_none());
}
#[test]
fn minted_challenge_explicitly_clears_sensitive_bytes() {
    let mut minted = MintedChallenge {
        frames: vec![vec![0xA5; 32]],
        admission: None,
    };
    minted.clear_sensitive_bytes();
    assert!(minted.frames.is_empty());
}
#[test]
fn mint_challenge_ticket_reports_rng_failure() {
    let pow_params = PowParameters::new(5, Duration::from_secs(900), Duration::from_secs(120));
    let config = SoranetHandshakeConfig::new(
        iroha_crypto::soranet::handshake::DEFAULT_DESCRIPTOR_COMMIT.to_vec(),
        iroha_crypto::soranet::handshake::DEFAULT_CLIENT_CAPABILITIES.to_vec(),
        iroha_crypto::soranet::handshake::DEFAULT_RELAY_CAPABILITIES.to_vec(),
        true,
        1,
        1,
        None,
        true,
        pow_params,
        None,
        Duration::from_secs(240),
        None,
        None,
        None,
    )
    .expect("test SoraNet handshake config must be valid");
    let mut rng = FailingTryRng;
    let transcript = test_admission_transcript();
    let err = config
        .mint_challenge_ticket(&transcript, &mut rng)
        .expect_err("failing RNG must abort challenge minting");
    match err {
        ChallengeMintError::Pow(pow::MintError::RandomBytes { operation, message }) => {
            assert_eq!(operation, "minting PoW solution nonce");
            assert!(
                message.contains("failing p2p ticket RNG"),
                "unexpected message: {message}"
            );
        }
        other => panic!("expected PoW RNG failure, got {other:?}"),
    }
}
#[test]
fn pow_ticket_replay_rejected_and_persisted() {
    let pow_params = PowParameters::new(1, Duration::from_secs(900), Duration::from_secs(120));
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("revocations.norito");
    let limits = TicketRevocationStoreLimits::new(4, Duration::from_secs(900)).expect("limits");
    let store = TicketRevocationStore::load(&path, limits, SystemTime::now()).expect("store");
    let config = SoranetHandshakeConfig::new(
        iroha_crypto::soranet::handshake::DEFAULT_DESCRIPTOR_COMMIT.to_vec(),
        iroha_crypto::soranet::handshake::DEFAULT_CLIENT_CAPABILITIES.to_vec(),
        iroha_crypto::soranet::handshake::DEFAULT_RELAY_CAPABILITIES.to_vec(),
        true,
        1,
        1,
        None,
        true,
        pow_params,
        None,
        Duration::from_secs(240),
        None,
        Some(Arc::new(Mutex::new(store))),
        None,
    )
    .expect("test SoraNet handshake config must be valid");
    let mut rng = StdRng::from_seed([0x21; 32]);
    let transcript = test_admission_transcript();
    let minted = config
        .mint_challenge_ticket(&transcript, &mut rng)
        .expect("mint")
        .expect("ticket present");
    let ticket = minted.frames.into_iter().next().expect("ticket frame");
    config
        .verify_challenge_ticket(&ticket, &transcript)
        .expect("first verify");
    let err = config
        .verify_challenge_ticket(&ticket, &transcript)
        .expect_err("replay must fail");
    assert!(matches!(err, ChallengeVerifyError::Replay));
    drop(config);
    let reloaded =
        TicketRevocationStore::load(&path, limits, SystemTime::now()).expect("reload store");
    let config_reloaded = SoranetHandshakeConfig::new(
        iroha_crypto::soranet::handshake::DEFAULT_DESCRIPTOR_COMMIT.to_vec(),
        iroha_crypto::soranet::handshake::DEFAULT_CLIENT_CAPABILITIES.to_vec(),
        iroha_crypto::soranet::handshake::DEFAULT_RELAY_CAPABILITIES.to_vec(),
        true,
        1,
        1,
        None,
        true,
        pow_params,
        None,
        Duration::from_secs(240),
        None,
        Some(Arc::new(Mutex::new(reloaded))),
        None,
    )
    .expect("test SoraNet handshake config must be valid");
    let err = config_reloaded
        .verify_challenge_ticket(&ticket, &transcript)
        .expect_err("replay after reload must fail");
    assert!(matches!(err, ChallengeVerifyError::Replay));
}
#[test]
fn signed_ticket_replay_persists_across_reload() {
    let pow_params = PowParameters::new(1, Duration::from_secs(300), Duration::from_secs(60));
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("signed_revocations.norito");
    let limits = TicketRevocationStoreLimits::new(8, Duration::from_secs(900)).expect("limits");
    let store = TicketRevocationStore::load(&path, limits, SystemTime::now()).expect("store");
    let keypair = generate_mldsa_keypair(MlDsaSuite::MlDsa44).expect("keygen");
    let config = SoranetHandshakeConfig::new(
        iroha_crypto::soranet::handshake::DEFAULT_DESCRIPTOR_COMMIT.to_vec(),
        iroha_crypto::soranet::handshake::DEFAULT_CLIENT_CAPABILITIES.to_vec(),
        iroha_crypto::soranet::handshake::DEFAULT_RELAY_CAPABILITIES.to_vec(),
        true,
        1,
        1,
        None,
        true,
        pow_params,
        None,
        Duration::from_secs(180),
        Some(keypair.public_key().to_vec()),
        Some(Arc::new(Mutex::new(store))),
        None,
    )
    .expect("test SoraNet handshake config must be valid");
    let mut rng = StdRng::from_seed([0x27; 32]);
    let transcript = test_admission_transcript();
    let ticket = pow::mint_ticket(
        config.pow_params.as_ref(),
        &config.pow_binding(&transcript),
        config.pow_ticket_ttl(),
        &mut rng,
    )
    .expect("mint pow ticket");
    let signed = SignedTicket::sign(
        ticket,
        &iroha_crypto::soranet::handshake::DEFAULT_DESCRIPTOR_COMMIT,
        &transcript,
        keypair.secret_key(),
    )
    .expect("sign ticket");
    let signed_bytes = signed.encode();
    config
        .verify_challenge_ticket(&signed_bytes, &transcript)
        .expect("first verify signed ticket");
    drop(config);
    let reloaded =
        TicketRevocationStore::load(&path, limits, SystemTime::now()).expect("reload store");
    let config_reloaded = SoranetHandshakeConfig::new(
        iroha_crypto::soranet::handshake::DEFAULT_DESCRIPTOR_COMMIT.to_vec(),
        iroha_crypto::soranet::handshake::DEFAULT_CLIENT_CAPABILITIES.to_vec(),
        iroha_crypto::soranet::handshake::DEFAULT_RELAY_CAPABILITIES.to_vec(),
        true,
        1,
        1,
        None,
        true,
        pow_params,
        None,
        Duration::from_secs(180),
        Some(keypair.public_key().to_vec()),
        Some(Arc::new(Mutex::new(reloaded))),
        None,
    )
    .expect("test SoraNet handshake config must be valid");
    let err = config_reloaded
        .verify_challenge_ticket(&signed_bytes, &transcript)
        .expect_err("signed ticket replay after reload must fail");
    assert!(matches!(err, ChallengeVerifyError::Replay));
}
#[test]
fn revocation_store_capacity_fails_closed_without_forgetting_replays() {
    let pow_params = PowParameters::new(1, Duration::from_secs(300), Duration::from_secs(60));
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("revocations.norito");
    let limits = TicketRevocationStoreLimits::new(1, Duration::from_secs(900)).expect("limits");
    let store = TicketRevocationStore::load(&path, limits, SystemTime::now()).expect("store");
    let config = SoranetHandshakeConfig::new(
        iroha_crypto::soranet::handshake::DEFAULT_DESCRIPTOR_COMMIT.to_vec(),
        iroha_crypto::soranet::handshake::DEFAULT_CLIENT_CAPABILITIES.to_vec(),
        iroha_crypto::soranet::handshake::DEFAULT_RELAY_CAPABILITIES.to_vec(),
        true,
        1,
        1,
        None,
        true,
        pow_params,
        None,
        Duration::from_secs(120),
        None,
        Some(Arc::new(Mutex::new(store))),
        None,
    )
    .expect("test SoraNet handshake config must be valid");
    let mut rng = StdRng::from_seed([0x31; 32]);
    let transcript = test_admission_transcript();
    let first = config
        .mint_challenge_ticket(&transcript, &mut rng)
        .expect("mint")
        .expect("ticket");
    let second = config
        .mint_challenge_ticket(&transcript, &mut rng)
        .expect("mint second")
        .expect("ticket");
    config
        .verify_challenge_ticket(&first.frames[0], &transcript)
        .expect("first verify");
    assert_eq!(config.active_revocations(), 1);
    let capacity_err = config
        .verify_challenge_ticket(&second.frames[0], &transcript)
        .expect_err("full store must fail closed");
    assert!(matches!(
        capacity_err,
        ChallengeVerifyError::RevocationStore(_)
    ));
    assert_eq!(
        config.active_revocations(),
        1,
        "capacity-one store must retain the first consumption record"
    );
    let replay_err = config
        .verify_challenge_ticket(&first.frames[0], &transcript)
        .expect_err("first ticket must remain consumed");
    assert!(matches!(replay_err, ChallengeVerifyError::Replay));
    config.purge_expired_revocations().expect("purge succeeds");
    assert_eq!(
        config.active_revocations(),
        1,
        "purge should not drop non-expired entries"
    );
}
#[test]
fn revocation_store_ttl_overflow_surfaces_store_error() {
    let pow_params = PowParameters::new(1, Duration::from_secs(300), Duration::from_secs(60));
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("revocations.norito");
    let limits = TicketRevocationStoreLimits::new(8, Duration::from_secs(10)).expect("limits");
    let store = TicketRevocationStore::load(&path, limits, SystemTime::now()).expect("store");
    let config = SoranetHandshakeConfig::new(
        iroha_crypto::soranet::handshake::DEFAULT_DESCRIPTOR_COMMIT.to_vec(),
        iroha_crypto::soranet::handshake::DEFAULT_CLIENT_CAPABILITIES.to_vec(),
        iroha_crypto::soranet::handshake::DEFAULT_RELAY_CAPABILITIES.to_vec(),
        true,
        1,
        1,
        None,
        true,
        pow_params,
        None,
        Duration::from_secs(120),
        None,
        Some(Arc::new(Mutex::new(store))),
        None,
    )
    .expect("test SoraNet handshake config must be valid");
    let mut rng = StdRng::from_seed([0x41; 32]);
    let transcript = test_admission_transcript();
    let minted = config
        .mint_challenge_ticket(&transcript, &mut rng)
        .expect("mint")
        .expect("ticket");
    let err = config
        .verify_challenge_ticket(&minted.frames[0], &transcript)
        .expect_err("revocation store ttl cap should reject ticket");
    assert!(matches!(err, ChallengeVerifyError::RevocationStore(_)));
}
#[test]
fn signed_ticket_invalid_signature_rejected() {
    let pow_params = PowParameters::new(1, Duration::from_secs(300), Duration::from_secs(60));
    let limits = TicketRevocationStoreLimits::new(8, Duration::from_secs(600)).expect("limits");
    let store =
        TicketRevocationStore::in_memory(limits).expect("revocation store should be available");
    let config = SoranetHandshakeConfig::new(
        iroha_crypto::soranet::handshake::DEFAULT_DESCRIPTOR_COMMIT.to_vec(),
        iroha_crypto::soranet::handshake::DEFAULT_CLIENT_CAPABILITIES.to_vec(),
        iroha_crypto::soranet::handshake::DEFAULT_RELAY_CAPABILITIES.to_vec(),
        true,
        1,
        1,
        None,
        true,
        pow_params,
        None,
        Duration::from_secs(120),
        None,
        Some(Arc::new(Mutex::new(store))),
        None,
    )
    .expect("test SoraNet handshake config must be valid");
    let keypair = generate_mldsa_keypair(MlDsaSuite::MlDsa44).expect("keygen");
    let expires_at = std::time::SystemTime::now()
        .checked_add(Duration::from_secs(120))
        .expect("ticket expiry should be representable")
        .duration_since(std::time::UNIX_EPOCH)
        .expect("current time should be after unix epoch")
        .as_secs();
    let ticket = PowTicket {
        version: 1,
        difficulty: 1,
        expires_at,
        client_nonce: [0u8; 32],
        solution: [0u8; 32],
    };
    let signed = SignedTicket {
        ticket,
        relay_id: config.relay_id.as_slice().try_into().unwrap(),
        transcript_hash: test_admission_transcript(),
        signature: vec![0x11; MlDsaSuite::MlDsa44.signature_len()],
    };
    let signed_bytes = signed.encode();
    let err = config
        .verify_signed_ticket(
            &signed_bytes,
            keypair.public_key(),
            &test_admission_transcript(),
        )
        .expect_err("invalid signature must fail");
    match err {
        ChallengeVerifyError::Pow(pow_err) => {
            assert!(matches!(pow_err, pow::Error::InvalidSignature))
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
#[test]
fn signed_ticket_with_config_key_accepts_once() {
    let pow_params = PowParameters::new(1, Duration::from_secs(300), Duration::from_secs(60));
    let limits = TicketRevocationStoreLimits::new(8, Duration::from_secs(900)).expect("limits");
    let store =
        TicketRevocationStore::in_memory(limits).expect("revocation store should be available");
    let keypair = generate_mldsa_keypair(MlDsaSuite::MlDsa44).expect("keygen");
    let config = SoranetHandshakeConfig::new(
        iroha_crypto::soranet::handshake::DEFAULT_DESCRIPTOR_COMMIT.to_vec(),
        iroha_crypto::soranet::handshake::DEFAULT_CLIENT_CAPABILITIES.to_vec(),
        iroha_crypto::soranet::handshake::DEFAULT_RELAY_CAPABILITIES.to_vec(),
        true,
        1,
        1,
        None,
        true,
        pow_params,
        None,
        Duration::from_secs(120),
        Some(keypair.public_key().to_vec()),
        Some(Arc::new(Mutex::new(store))),
        None,
    )
    .expect("test SoraNet handshake config must be valid");
    let mut rng = StdRng::from_seed([0x55; 32]);
    let transcript = test_admission_transcript();
    let ticket = pow::mint_ticket(
        config.pow_params.as_ref(),
        &config.pow_binding(&transcript),
        config.pow_ticket_ttl(),
        &mut rng,
    )
    .expect("mint pow ticket");
    let signed = SignedTicket::sign(
        ticket,
        &iroha_crypto::soranet::handshake::DEFAULT_DESCRIPTOR_COMMIT,
        &transcript,
        keypair.secret_key(),
    )
    .expect("sign ticket");
    let signed_bytes = signed.encode();
    let admission = config
        .verify_challenge_ticket(&signed_bytes, &transcript)
        .expect("verify signed ticket")
        .expect("admission");
    assert_eq!(admission.pow.difficulty(), pow_params.difficulty());
    let err = config
        .verify_challenge_ticket(&signed_bytes, &transcript)
        .expect_err("replay should be rejected");
    assert!(matches!(err, ChallengeVerifyError::Replay));
}
#[test]
fn raw_ticket_rejected_with_signed_key_present() {
    let pow_params = PowParameters::new(1, Duration::from_secs(300), Duration::from_secs(60));
    let limits = TicketRevocationStoreLimits::new(4, Duration::from_secs(900)).expect("limits");
    let store =
        TicketRevocationStore::in_memory(limits).expect("revocation store should be available");
    let keypair = generate_mldsa_keypair(MlDsaSuite::MlDsa44).expect("keygen");
    let config = SoranetHandshakeConfig::new(
        iroha_crypto::soranet::handshake::DEFAULT_DESCRIPTOR_COMMIT.to_vec(),
        iroha_crypto::soranet::handshake::DEFAULT_CLIENT_CAPABILITIES.to_vec(),
        iroha_crypto::soranet::handshake::DEFAULT_RELAY_CAPABILITIES.to_vec(),
        true,
        1,
        1,
        None,
        true,
        pow_params,
        None,
        Duration::from_secs(120),
        Some(keypair.public_key().to_vec()),
        Some(Arc::new(Mutex::new(store))),
        None,
    )
    .expect("test SoraNet handshake config must be valid");
    let mut rng = StdRng::from_seed([0xA5; 32]);
    let transcript = test_admission_transcript();
    let ticket = pow::mint_ticket(
        config.pow_params.as_ref(),
        &config.pow_binding(&transcript),
        config.pow_ticket_ttl(),
        &mut rng,
    )
    .expect("mint pow ticket");
    let ticket_bytes = ticket.to_vec();
    let err = config
        .verify_challenge_ticket(&ticket_bytes, &transcript)
        .expect_err("raw ticket must fail when signed-ticket key is configured");
    assert!(matches!(
        err,
        ChallengeVerifyError::Pow(pow::Error::Malformed(_))
    ));
}
#[test]
fn signed_challenge_ticket_rejects_client_hello_substitution_before_signature_work() {
    let pow_params = PowParameters::new(1, Duration::from_secs(300), Duration::from_secs(60));
    let config = SoranetHandshakeConfig::new(
        iroha_crypto::soranet::handshake::DEFAULT_DESCRIPTOR_COMMIT.to_vec(),
        iroha_crypto::soranet::handshake::DEFAULT_CLIENT_CAPABILITIES.to_vec(),
        iroha_crypto::soranet::handshake::DEFAULT_RELAY_CAPABILITIES.to_vec(),
        true,
        1,
        1,
        None,
        true,
        pow_params,
        None,
        Duration::from_secs(120),
        Some(vec![0x77]),
        None,
        None,
    )
    .expect("test SoraNet handshake config must be valid");
    let transcript = test_admission_transcript();
    let substituted = substituted_admission_transcript();
    let expires_at = SystemTime::now()
        .checked_add(Duration::from_secs(120))
        .expect("expiry should be representable")
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock should be after the unix epoch")
        .as_secs();
    let signed = SignedTicket {
        ticket: PowTicket {
            version: PowTicket::VERSION,
            difficulty: 1,
            expires_at,
            client_nonce: [0x44; 32],
            solution: [0x55; 32],
        },
        relay_id: config.relay_id.as_slice().try_into().expect("relay id"),
        transcript_hash: transcript,
        signature: vec![0x66; MlDsaSuite::MlDsa44.signature_len()],
    };
    let err = config
        .verify_challenge_ticket(&signed.encode(), &substituted)
        .expect_err("signed ticket must be bound to the exact client hello");
    assert!(matches!(
        err,
        ChallengeVerifyError::Pow(pow::Error::TranscriptMismatch)
    ));
}
#[tokio::test(flavor = "current_thread")]
async fn puzzle_work_is_offloaded_serialized_and_remains_bounded_after_cancellation() {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        mpsc as std_mpsc,
    };
    let gate = Arc::new(Semaphore::new(1));
    let first_started = Arc::new(AtomicBool::new(false));
    let second_started = Arc::new(AtomicBool::new(false));
    let (release_first, wait_for_release) = std_mpsc::channel();
    let first_started_by_work = Arc::clone(&first_started);
    let first = tokio::spawn(run_soranet_puzzle_work(Arc::clone(&gate), move || {
        first_started_by_work.store(true, Ordering::Release);
        wait_for_release
            .recv_timeout(Duration::from_secs(2))
            .map_err(|error| Error::HandshakeSoranet(error.to_string()))?;
        Ok(1_u8)
    }));
    tokio::time::timeout(Duration::from_secs(1), async {
        while !first_started.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("blocking puzzle work must not stall the current-thread async executor");
    // Dropping the handshake future cannot cancel spawn_blocking. The
    // blocking task must therefore retain the sole permit until it really
    // exits, or a reconnect would recreate the original puzzle storm.
    first.abort();
    let _ = first.await;
    let cancelled_waiter_started = Arc::new(AtomicBool::new(false));
    let cancelled_waiter_started_by_work = Arc::clone(&cancelled_waiter_started);
    let cancelled_waiter = tokio::spawn(run_soranet_puzzle_work(Arc::clone(&gate), move || {
        cancelled_waiter_started_by_work.store(true, Ordering::Release);
        Ok(3_u8)
    }));
    tokio::task::yield_now().await;
    assert!(
        !cancelled_waiter_started.load(Ordering::Acquire),
        "a queued handshake must apply backpressure before blocking work starts"
    );
    cancelled_waiter.abort();
    let _ = cancelled_waiter.await;
    let second_started_by_work = Arc::clone(&second_started);
    let second = tokio::spawn(run_soranet_puzzle_work(Arc::clone(&gate), move || {
        second_started_by_work.store(true, Ordering::Release);
        Ok(2_u8)
    }));
    tokio::time::sleep(Duration::from_millis(25)).await;
    assert!(
        !second_started.load(Ordering::Acquire),
        "a retry cannot overlap uncancellable blocking puzzle work"
    );
    release_first.send(()).expect("release first puzzle work");
    let result = tokio::time::timeout(Duration::from_secs(1), second)
        .await
        .expect("serialized retry should start after the first task exits")
        .expect("retry task should not panic")
        .expect("retry puzzle work should succeed");
    assert_eq!(result, 2);
    assert!(second_started.load(Ordering::Acquire));
    assert!(
        !cancelled_waiter_started.load(Ordering::Acquire),
        "disconnecting while queued must remove work before it reaches Argon2"
    );
}
#[tokio::test(flavor = "current_thread")]
async fn puzzle_work_gate_bounds_concurrency_and_keeps_the_async_runtime_responsive() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    const WORKERS: usize = 8;
    let gate = Arc::new(Semaphore::new(1));
    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let mut workers = Vec::with_capacity(WORKERS);
    for value in 0..WORKERS {
        let active = Arc::clone(&active);
        let peak = Arc::clone(&peak);
        workers.push(tokio::spawn(run_soranet_puzzle_work(
            Arc::clone(&gate),
            move || {
                let current = active.fetch_add(1, Ordering::AcqRel) + 1;
                peak.fetch_max(current, Ordering::AcqRel);
                std::thread::sleep(Duration::from_millis(10));
                active.fetch_sub(1, Ordering::AcqRel);
                Ok(value)
            },
        )));
    }
    let heartbeat = tokio::spawn(async {
        for _ in 0..WORKERS {
            tokio::task::yield_now().await;
        }
    });
    tokio::time::timeout(Duration::from_secs(2), heartbeat)
        .await
        .expect("blocking puzzle work must not starve the async executor")
        .expect("heartbeat task must not panic");
    let completed = tokio::time::timeout(Duration::from_secs(2), async {
        let mut values = Vec::with_capacity(WORKERS);
        for worker in workers {
            values.push(
                worker
                    .await
                    .expect("puzzle worker task must not panic")
                    .expect("puzzle worker must succeed"),
            );
        }
        values
    })
    .await
    .expect("backpressured workers should eventually complete");
    assert_eq!(completed.len(), WORKERS);
    assert_eq!(peak.load(Ordering::Acquire), 1);
    assert_eq!(active.load(Ordering::Acquire), 0);
    assert_eq!(gate.available_permits(), 1);
}
#[tokio::test(flavor = "current_thread")]
async fn inbound_puzzle_pressure_cannot_consume_outbound_recovery_capacity() {
    let admission = SoranetPuzzleWorkAdmission::new(
        NonZeroUsize::new(1).expect("non-zero outbound capacity"),
        NonZeroUsize::new(1).expect("non-zero inbound capacity"),
    );
    let inbound_gate = admission.inbound_verify_gate();
    let held_inbound = inbound_gate
        .clone()
        .acquire_owned()
        .await
        .expect("inbound gate open");
    assert!(inbound_gate.try_acquire_owned().is_err());
    let outbound = admission
        .outbound_mint_gate()
        .try_acquire_owned()
        .expect("inbound verification cannot starve outbound ticket minting");
    drop(outbound);
    drop(held_inbound);
    assert_eq!(admission.inbound_verify_gate().available_permits(), 1);
    assert_eq!(admission.outbound_mint_gate().available_permits(), 1);
}
#[tokio::test(flavor = "current_thread")]
async fn closed_puzzle_work_gate_fails_closed_without_running_work() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let gate = Arc::new(Semaphore::new(1));
    gate.close();
    let started = Arc::new(AtomicBool::new(false));
    let started_by_work = Arc::clone(&started);
    let error = run_soranet_puzzle_work(gate, move || {
        started_by_work.store(true, Ordering::Release);
        Ok(())
    })
    .await
    .expect_err("a closed puzzle gate must reject work");
    assert!(matches!(
        error,
        Error::HandshakeSoranet(message)
            if message.starts_with("SoraNet puzzle work gate closed:")
    ));
    assert!(!started.load(Ordering::Acquire));
}
#[tokio::test(flavor = "current_thread")]
async fn ordinary_pow_verification_does_not_depend_on_the_puzzle_gate() {
    let config = Arc::new(
        SoranetHandshakeConfig::new(
            iroha_crypto::soranet::handshake::DEFAULT_DESCRIPTOR_COMMIT.to_vec(),
            iroha_crypto::soranet::handshake::DEFAULT_CLIENT_CAPABILITIES.to_vec(),
            iroha_crypto::soranet::handshake::DEFAULT_RELAY_CAPABILITIES.to_vec(),
            true,
            1,
            1,
            None,
            true,
            PowParameters::new(1, Duration::from_secs(30), Duration::from_secs(1)),
            None,
            Duration::from_secs(5),
            None,
            None,
            None,
        )
        .expect("test SoraNet handshake config must be valid"),
    );
    let transcript_hash = test_admission_transcript();
    let ticket = mint_test_admission_ticket(&config, &transcript_hash, [0x30; 32]);
    let closed_gate = Arc::new(Semaphore::new(1));
    closed_gate.close();
    let admission =
        verify_handshake_challenge_with_gate(config, ticket, transcript_hash, closed_gate)
            .await
            .expect("ordinary PoW should preserve its direct verification path")
            .expect("ordinary PoW should return admission policy");
    assert_eq!(admission.pow.difficulty(), 1);
    assert!(admission.puzzle.is_none());
}
#[tokio::test(flavor = "current_thread")]
async fn signed_ticket_verification_does_not_depend_on_the_puzzle_gate() {
    let keypair = generate_mldsa_keypair(MlDsaSuite::MlDsa44).expect("keygen");
    let max_future_skew = Duration::from_secs(30);
    let min_ticket_ttl = Duration::from_secs(1);
    let config = Arc::new(
        SoranetHandshakeConfig::new(
            iroha_crypto::soranet::handshake::DEFAULT_DESCRIPTOR_COMMIT.to_vec(),
            iroha_crypto::soranet::handshake::DEFAULT_CLIENT_CAPABILITIES.to_vec(),
            iroha_crypto::soranet::handshake::DEFAULT_RELAY_CAPABILITIES.to_vec(),
            true,
            1,
            1,
            None,
            true,
            PowParameters::new(1, max_future_skew, min_ticket_ttl),
            Some(PuzzleParameters::new(
                NonZeroU32::new(puzzle::MIN_MEMORY_KIB).expect("minimum puzzle memory is non-zero"),
                NonZeroU32::new(1).expect("one Argon2 iteration is non-zero"),
                NonZeroU32::new(1).expect("one Argon2 lane is non-zero"),
                1,
                max_future_skew,
                min_ticket_ttl,
            )),
            Duration::from_secs(5),
            Some(keypair.public_key().to_vec()),
            None,
            None,
        )
        .expect("test SoraNet handshake config must be valid"),
    );
    let transcript_hash = test_admission_transcript();
    let ticket = pow::mint_ticket(
        config.pow_params.as_ref(),
        &config.pow_binding(&transcript_hash),
        config.pow_ticket_ttl(),
        &mut StdRng::from_seed([0x34; 32]),
    )
    .expect("test PoW ticket should mint");
    let signed = SignedTicket::sign(
        ticket,
        &iroha_crypto::soranet::handshake::DEFAULT_DESCRIPTOR_COMMIT,
        &transcript_hash,
        keypair.secret_key(),
    )
    .expect("test ticket should sign")
    .encode();
    let closed_gate = Arc::new(Semaphore::new(1));
    closed_gate.close();
    let admission =
        verify_handshake_challenge_with_gate(config, signed, transcript_hash, closed_gate)
            .await
            .expect("signed tickets must stay on their non-Argon verification path")
            .expect("signed ticket should return admission policy");
    assert_eq!(admission.pow.difficulty(), 1);
}
#[tokio::test(flavor = "current_thread")]
async fn inbound_puzzle_verification_accepts_a_fresh_valid_ticket() {
    let config = minimal_puzzle_config(
        Duration::from_secs(5),
        Duration::from_secs(30),
        Duration::from_secs(1),
    );
    let transcript_hash = test_admission_transcript();
    let ticket = mint_test_admission_ticket(&config, &transcript_hash, [0x31; 32]);
    let admission = tokio::time::timeout(
        Duration::from_secs(2),
        verify_handshake_challenge_with_gate(
            config,
            ticket,
            transcript_hash,
            Arc::new(Semaphore::new(1)),
        ),
    )
    .await
    .expect("inbound verification should complete")
    .expect("fresh valid puzzle ticket should verify")
    .expect("puzzle verification should return admission policy");
    assert_eq!(admission.pow.difficulty(), 1);
    assert!(admission.puzzle.is_some());
}
#[tokio::test(flavor = "current_thread")]
async fn inbound_puzzle_verification_rejects_an_invalid_ticket() {
    let config = minimal_puzzle_config(
        Duration::from_secs(5),
        Duration::from_secs(30),
        Duration::from_secs(1),
    );
    let transcript_hash = test_admission_transcript();
    let mut ticket = mint_test_admission_ticket(&config, &transcript_hash, [0x32; 32]);
    ticket[0] ^= 0xFF;
    let error = verify_handshake_challenge_with_gate(
        config,
        ticket,
        transcript_hash,
        Arc::new(Semaphore::new(1)),
    )
    .await
    .expect_err("malformed inbound puzzle ticket must be rejected");
    assert!(matches!(
        error,
        Error::HandshakeSoranet(message)
            if message.contains("unsupported pow ticket version")
    ));
}
#[tokio::test(flavor = "current_thread")]
async fn inbound_puzzle_ticket_expiring_while_queued_is_rejected() {
    let config = minimal_puzzle_config(
        Duration::from_secs(3),
        Duration::from_secs(10),
        Duration::from_secs(1),
    );
    let transcript_hash = test_admission_transcript();
    let ticket = mint_test_admission_ticket(&config, &transcript_hash, [0x33; 32]);
    config
        .verify_challenge_ticket(&ticket, &transcript_hash)
        .expect("ticket must be valid before it enters the work queue");
    let expires_at = PowTicket::parse(&ticket)
        .expect("minted ticket must parse")
        .expires_at;
    let gate = Arc::new(Semaphore::new(1));
    let occupied = Arc::clone(&gate)
        .acquire_owned()
        .await
        .expect("test gate must be open");
    let queued = tokio::spawn(verify_handshake_challenge_with_gate(
        config,
        ticket,
        transcript_hash,
        Arc::clone(&gate),
    ));
    tokio::task::yield_now().await;
    assert_eq!(gate.available_permits(), 0);
    let now_secs = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock must be after the Unix epoch")
        .as_secs();
    tokio::time::sleep(Duration::from_secs(
        expires_at.saturating_sub(now_secs).saturating_add(1),
    ))
    .await;
    drop(occupied);
    let error = tokio::time::timeout(Duration::from_secs(1), queued)
        .await
        .expect("expired queued verification should finish promptly")
        .expect("queued verification task must not panic")
        .expect_err("ticket freshness must be checked after queue admission");
    assert!(matches!(
        error,
        Error::HandshakeSoranet(message) if message.contains("puzzle ticket expired")
    ));
}
