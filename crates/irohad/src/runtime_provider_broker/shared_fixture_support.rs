/// Builds the deterministic network identifier used by broker tests.
fn test_network_id(byte: u8) -> NetworkId {
    network_id_from(byte)
}

/// Returns the canonical network identifier used by broker server fixtures.
fn server_test_network_id() -> NetworkId {
    network_id()
}

/// Returns the checked Ed25519 keypair matching `TEST_SIGNER_KEY`.
fn test_governance_signer_keypair() -> KeyPair {
    let keypair = KeyPair::try_from_seed(
        hex::decode("3a7991af1abb77f3fd27cc148404a6ae4439d095a63591b77c788d53f708a02a")
            .expect("decode governance signer test seed"),
        Algorithm::Ed25519,
    )
    .expect("derive governance signer test keypair");
    assert_eq!(keypair.public_key().to_bytes().1, TEST_SIGNER_KEY);
    keypair
}

/// Signs one exact governance fixture payload with the configured test key.
fn test_governance_signature(payload: &[u8]) -> [u8; 64] {
    Signature::try_new(test_governance_signer_keypair().private_key(), payload)
        .expect("sign governance fixture payload")
        .payload()
        .try_into()
        .expect("Ed25519 signatures are exactly 64 bytes")
}

/// Signs the exact inner payload carried by a governance broker request.
fn test_governance_operation_signature(request: &OperationRequestV1) -> [u8; 64] {
    let signing = decode_canonical::<PurposeSignRequestWireV1>(
        &request.payload,
        MAX_OPERATION_FRAME_BYTES_V1,
    )
    .expect("decode governance signing request");
    test_governance_signature(&signing.payload)
}

/// Returns the deterministic request-auth signing key used by broker tests.
fn server_test_request_auth_keypair() -> iroha_crypto::KeyPair {
    test_auth_keypair()
}

/// Returns the Ed25519 request-auth public key in its broker wire representation.
fn server_test_request_auth_public_key() -> [u8; 32] {
    let keypair = server_test_request_auth_keypair();
    let public_key = keypair.public_key().to_bytes().1;
    let mut bytes = [0_u8; 32];
    bytes.copy_from_slice(public_key);
    bytes
}

/// Builds the canonical governance request-ingress binding fixture.
fn server_test_request_ingress_binding(
    public_key: [u8; 32],
) -> sorafs_node::GovernanceDagRequestIngressBindingV1 {
    ingress_fixture(public_key)
}

/// Returns the qualified governance request-auth provider catalog fixture.
fn request_auth_server_test_catalog() -> IrohaRuntimeProviderBindingsV1 {
    request_auth_catalog()
}

/// Returns the exact governance request-auth backend fixture.
fn request_auth_server_test_backends() -> RuntimeProviderBrokerBackendsV1 {
    request_auth_backends()
}

/// Returns the single proof-outcome native signer catalog fixture.
fn proof_native_signer_test_catalog() -> IrohaRuntimeProviderBindingsV1 {
    signer_catalog()
}

/// Builds a deterministic durable moderation handoff request fixture.
fn moderation_handoff_test_request(
    kind: sorafs_node::moderation_orchestrator::ModerationTerminalHandoffKindV1,
) -> iroha_torii::sorafs::moderation_runtime::ModerationDurableHandoffRequestV1 {
    moderation_handoff_request(kind)
}

/// Builds a deterministic durable moderation panel-notification request fixture.
fn moderation_panel_test_request()
-> iroha_torii::sorafs::moderation_runtime::ModerationDurablePanelNotificationRequestV1 {
    moderation_panel_request()
}

/// Returns the qualified moderation quarantine provider catalog fixture.
fn moderation_server_test_catalog() -> IrohaRuntimeProviderBindingsV1 {
    moderation_catalog()
}

/// Returns the qualified catalog fixture for a reputation runtime slot.
fn reputation_runtime_test_catalog(
    slot: IrohaRuntimeProviderSlotV1,
) -> IrohaRuntimeProviderBindingsV1 {
    reputation_catalog(slot)
}

/// Builds the deterministic reputation threshold-signing request fixture.
fn reputation_test_threshold_request()
-> sorafs_node::reputation::runtime::ReputationThresholdSigningRequestV1 {
    threshold_request()
}

/// Builds a provider binding without slot-specific optional metadata.
fn plain_runtime_binding(slot: IrohaRuntimeProviderSlotV1, handle: &str) -> ProviderBindingWireV1 {
    runtime_binding(slot, handle)
}

/// Recomputes a mutated provider observation's canonical metadata digest.
fn refresh_metadata_digest(observed: &mut ProviderObservationWireV1) {
    metadata_digest(observed);
}

/// Builds and validates a canonical broker operation fixture.
fn validated_test_operation(
    binding: ProviderBindingWireV1,
    operation: u16,
    payload: Vec<u8>,
) -> OperationRequestV1 {
    validated_operation(binding, operation, payload)
}

/// Starts a native-signer broker with the supplied catalog and backends.
fn start_native_signer_server(
    bindings: IrohaRuntimeProviderBindingsV1,
    backends: RuntimeProviderBrokerBackendsV1,
) -> (
    tempfile::TempDir,
    EndpointPolicy,
    Arc<RuntimeProviderBrokerLifecycleV1>,
    thread::JoinHandle<Result<(), RuntimeProviderBrokerServerErrorV1>>,
) {
    start_signer(bindings, backends)
}

/// Projects a canonical provider binding for the requested reputation slot.
fn reputation_binding(slot: IrohaRuntimeProviderSlotV1) -> ProviderBindingWireV1 {
    let catalog = reputation_catalog(slot);
    ProviderBindingWireV1::try_from_binding(catalog.iter().next().expect("one reputation binding"))
        .expect("project reputation test binding")
}

/// Builds a valid finalized `PoR` replay-archive record fixture.
fn por_replay_archive_record_fixture() -> node::PorFinalizedReplayArchiveRecordV1 {
    use iroha_data_model::sorafs::reputation::{PorTerminalOutcomeV1, PorTerminalStatusV1};
    use sorafs_manifest::{
        por::{
            AUDIT_VERDICT_VERSION_V1, AuditOutcomeV1, AuditVerdictV1, POR_CHALLENGE_VERSION_V1,
            PorChallengeV1, derive_challenge_id, derive_challenge_seed,
        },
        provider_advert::{AdvertSignature, SignatureAlgorithm},
    };
    const ISSUED_AT: u64 = 1_700_000_000;
    const SUBMITTED_AT: u64 = 1_700_000_100;
    const DECIDED_AT: u64 = 1_700_000_300;
    const DEADLINE_AT: u64 = 1_700_000_600;
    let manifest_digest = [0x22; 32];
    let provider_id = [0x33; 32];
    let epoch_id = 123;
    let drand_round = 456;
    let drand_randomness = [0x41; 32];
    let seed = derive_challenge_seed(&drand_randomness, None, &manifest_digest, epoch_id);
    let challenge_id =
        derive_challenge_id(&seed, &manifest_digest, &provider_id, epoch_id, drand_round);
    let challenge = PorChallengeV1 {
        version: POR_CHALLENGE_VERSION_V1,
        challenge_id,
        manifest_digest,
        provider_id,
        epoch_id,
        drand_round,
        drand_randomness,
        drand_signature: [0x61; 48],
        vrf_output: None,
        vrf_proof: None,
        forced: true,
        chunking_profile: "sorafs.sf1@1.0.0".to_owned(),
        seed,
        sample_tier: 1,
        sample_count: 1,
        sample_indices: vec![0],
        issued_at: ISSUED_AT,
        deadline_at: DEADLINE_AT,
    };
    challenge
        .validate()
        .expect("valid replay-archive challenge");
    let proof_digest = [0x52; 32];
    let mut verdict = AuditVerdictV1 {
        version: AUDIT_VERDICT_VERSION_V1,
        manifest_digest,
        provider_id,
        challenge_id,
        proof_digest: Some(proof_digest),
        outcome: AuditOutcomeV1::Success,
        failure_reason: None,
        decided_at: DECIDED_AT,
        auditor_signatures: vec![AdvertSignature {
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: Vec::new(),
            signature: Vec::new(),
        }],
        metadata: Vec::new(),
    };
    let auditor = KeyPair::try_from_seed(vec![0x13; 32], Algorithm::Ed25519)
        .expect("replay-archive auditor keypair");
    let verdict_payload = verdict
        .signature_payload_bytes()
        .expect("encode replay-archive verdict signature payload");
    verdict.auditor_signatures[0].public_key = auditor.public_key().to_bytes().1.to_vec();
    verdict.auditor_signatures[0].signature =
        Signature::try_new(auditor.private_key(), &verdict_payload)
            .expect("sign replay-archive verdict")
            .payload()
            .to_vec();
    verdict.validate().expect("valid replay-archive verdict");
    verdict
        .verify_signatures()
        .expect("authenticated replay-archive verdict");
    let fixture = PorReplayArchiveRecordFixtureV1 {
        finalized: PorReplayArchiveFinalizedStateFixtureV1 {
            state: PorReplayArchiveChallengeStateFixtureV1 {
                challenge,
                proof_digest: Some(proof_digest),
                proof_submitted_at: Some(SUBMITTED_AT),
            },
            verdict,
            stats: node::PorVerdictStats {
                success_samples: 1,
                failed_samples: 0,
            },
            repair_task_id: None,
            reputation_sequence: 1,
            reputation_terminal: PorTerminalOutcomeV1 {
                challenge_id,
                manifest_digest,
                epoch_id,
                drand_round,
                forced: true,
                sample_count: 1,
                failed_samples: 0,
                issued_at_unix_ms: ISSUED_AT * 1_000,
                deadline_at_unix_ms: DEADLINE_AT * 1_000,
                responded_at_unix_ms: Some(SUBMITTED_AT * 1_000),
                decided_at_unix_ms: DECIDED_AT * 1_000,
                proof_digest: Some(proof_digest),
                repair_task_id: None,
                verifier_latency_ms: Some(
                    u32::try_from((DECIDED_AT - SUBMITTED_AT) * 1_000)
                        .expect("test verifier latency fits u32"),
                ),
                status: PorTerminalStatusV1::Verified,
            },
        },
    };
    let canonical = encode_canonical(&fixture, MAX_POR_REPLAY_ARCHIVE_RECORD_BYTES_V1)
        .expect("encode replay-archive record fixture");
    decode_por_replay_archive_record(&canonical)
        .expect("fixture is the canonical production record layout")
}
