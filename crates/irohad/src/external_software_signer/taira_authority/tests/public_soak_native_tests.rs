use super::*;
use sha2::{Digest as _, Sha256};

const OBSERVATION_WRAPPING_KEY_V1: [u8; 32] = [0xD1; 32];
const REPLAY_WRAPPING_KEY_V1: [u8; 32] = [0xD2; 32];
const PUBLIC_SOAK_SUBJECT_DOMAIN_V1: &[u8] =
    b"iroha.taira.public-v2-24h-soak.authority-subject.v1\0";
const PUBLIC_SOAK_REPLAY_NAMESPACE_V1: &str = "iroha.taira.public-v2-24h-soak-authority-replay.v1";
const PUBLIC_SOAK_MAX_AUTHORITY_LIFETIME_MILLIS_V1: u64 = 15 * 60 * 1_000;

fn observation_wrapping_key() -> SoftwareSignerWrappingKeyV1 {
    SoftwareSignerWrappingKeyV1::try_from_bytes(OBSERVATION_WRAPPING_KEY_V1)
        .expect("observation wrapping key")
}

fn replay_wrapping_key() -> SoftwareSignerWrappingKeyV1 {
    SoftwareSignerWrappingKeyV1::try_from_bytes(REPLAY_WRAPPING_KEY_V1)
        .expect("replay wrapping key")
}

fn soak_provisioning(
    role: TairaAuthorityRoleV1,
    label: &str,
    service_uid_offset: u32,
    client_uid_offset: u32,
    administrator_uid_offset: u32,
    policy_byte: u8,
) -> TairaAuthorityProvisioningV1 {
    let effective_uid = rustix::process::geteuid().as_raw();
    let offset_uid = |offset: u32| {
        effective_uid
            .checked_add(offset)
            .expect("public-soak fixture UID range")
    };
    TairaAuthorityProvisioningV1 {
        role,
        service_id: format!("taira-authority-{}-{label}-v1", role.as_str()),
        administrator_id: format!("taira-authority-{}-{label}-administrator-v1", role.as_str()),
        service_uid: offset_uid(service_uid_offset),
        client_uid: offset_uid(client_uid_offset),
        administrator_uid: offset_uid(administrator_uid_offset),
        key_revision: 1,
        policy_revision: 1,
        policy_digest: [policy_byte; 32],
        max_request_bytes: 1024 * 1024,
    }
}

fn provision_observation(state: &Path) -> TairaAuthorityServiceV1 {
    TairaAuthorityServiceV1::provision_for_test(
        state,
        soak_provisioning(
            TairaAuthorityRoleV1::PublicSoakObservation,
            "native-observation",
            101,
            102,
            103,
            0x71,
        ),
        observation_wrapping_key(),
    )
    .expect("provision public-soak observation authority")
}

fn provision_replay(
    state: &Path,
    observation_binding: super::super::TairaAuthorityPublicBindingV1,
) -> TairaAuthorityServiceV1 {
    TairaAuthorityServiceV1::provision_with_public_soak_observation_binding_for_test(
        state,
        soak_provisioning(
            TairaAuthorityRoleV1::PublicSoakReplayAdmission,
            "native-replay",
            201,
            0,
            203,
            0x72,
        ),
        replay_wrapping_key(),
        observation_binding,
    )
    .expect("provision public-soak replay authority")
}

fn digest_value(byte: u8) -> Value {
    Value::from(hex::encode([byte; 32]))
}

fn digest_count(digest_byte: u8, digest_field: &str, count_field: &str, count: u64) -> Value {
    let mut value = Map::new();
    value.insert(digest_field.into(), digest_value(digest_byte));
    value.insert(count_field.into(), Value::from(count));
    Value::Object(value)
}

fn inventory(artifact_byte: u8, records_byte: u8, count: u64) -> Value {
    let mut value = Map::new();
    value.insert("artifact_sha256".into(), digest_value(artifact_byte));
    value.insert("record_count".into(), Value::from(count));
    value.insert("records_sha256".into(), digest_value(records_byte));
    Value::Object(value)
}

pub(super) fn valid_public_soak_subject_core() -> Value {
    let mut receipt = Map::new();
    receipt.insert("sha256".into(), digest_value(0x11));
    receipt.insert("size_bytes".into(), Value::from(8192_u64));

    let mut source = Map::new();
    source.insert("tuple_sha256".into(), digest_value(0x12));

    let mut prerequisites = Map::new();
    prerequisites.insert("candidate_handoff_sha256".into(), digest_value(0x13));
    prerequisites.insert("deploy_handoff_sha256".into(), digest_value(0x14));
    prerequisites.insert("publication_handoff_sha256".into(), digest_value(0x15));

    let mut lifecycle = Map::new();
    lifecycle.insert("artifact_sha256".into(), digest_value(0x20));
    lifecycle.insert("identity_sha256".into(), digest_value(0x21));
    lifecycle.insert("journal_artifact_sha256".into(), digest_value(0x22));
    lifecycle.insert("journal_record_count".into(), Value::from(1440_u64));
    lifecycle.insert("journal_records_sha256".into(), digest_value(0x23));
    lifecycle.insert("native_verifier_receipt_sha256".into(), digest_value(0x24));
    lifecycle.insert("window_sha256".into(), digest_value(0x25));

    let mut verifier = Map::new();
    verifier.insert("binary_sha256".into(), digest_value(0x26));
    verifier.insert("source_sha256".into(), digest_value(0x27));

    let mut subject = Map::new();
    subject.insert(
        "anchor".into(),
        digest_count(0x30, "sha256", "validator_count", 4),
    );
    subject.insert("applied_statuses".into(), inventory(0x31, 0x32, 2880));
    subject.insert("blocks".into(), inventory(0x33, 0x34, 1440));
    subject.insert("lifecycle".into(), Value::Object(lifecycle));
    subject.insert("native_verifier".into(), Value::Object(verifier));
    subject.insert("prerequisites".into(), Value::Object(prerequisites));
    subject.insert("receipt".into(), Value::Object(receipt));
    subject.insert(
        "samples".into(),
        digest_count(0x35, "sha256", "count", 8640),
    );
    subject.insert(
        "schema".into(),
        Value::from("iroha.taira.public-v2-24h-soak-authority-subject.v1"),
    );
    subject.insert("source".into(), Value::Object(source));
    subject.insert("submission_receipts".into(), inventory(0x36, 0x37, 2880));
    subject.insert("workload".into(), inventory(0x38, 0x39, 2880));
    Value::Object(subject)
}

fn public_soak_subject_digest(subject: &Value) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(PUBLIC_SOAK_SUBJECT_DOMAIN_V1);
    digest.update(canonical_json_line(subject));
    digest.finalize().into()
}

pub(super) fn observation_subject(core: &Value, completed_at_unix_millis: u64) -> Value {
    let mut subject = Map::new();
    subject.insert(
        "completed_at_unix_ms".into(),
        Value::from(completed_at_unix_millis),
    );
    subject.insert("subject".into(), core.clone());
    subject.insert(
        "subject_digest".into(),
        Value::from(hex::encode(public_soak_subject_digest(core))),
    );
    Value::Object(subject)
}

pub(super) fn replay_subject(
    core: &Value,
    completed_at_unix_millis: u64,
    authority_envelope: Value,
) -> Value {
    let envelope_json = canonical_json_line(&authority_envelope);
    let mut subject = Map::new();
    subject.insert("authority_envelope".into(), authority_envelope);
    subject.insert(
        "authority_envelope_sha256".into(),
        Value::from(hex::encode(Sha256::digest(&envelope_json))),
    );
    subject.insert(
        "completed_at_unix_ms".into(),
        Value::from(completed_at_unix_millis),
    );
    subject.insert(
        "replay_namespace".into(),
        Value::from(PUBLIC_SOAK_REPLAY_NAMESPACE_V1),
    );
    subject.insert("subject".into(), core.clone());
    subject.insert(
        "subject_digest".into(),
        Value::from(hex::encode(public_soak_subject_digest(core))),
    );
    Value::Object(subject)
}

fn assign_window(
    service: &TairaAuthorityServiceV1,
    fixture: &ClientRequestFixtureV1,
    not_before: u64,
    expires_at: u64,
) {
    let assigned = service
        .assign_run_json(
            &fixture.assignment_json(
                service,
                not_before.saturating_sub(10),
                not_before,
                expires_at,
            ),
            not_before,
        )
        .expect("assign public-soak run");
    assert_eq!(assigned.status, OperationStatusV1::Ok);
}

fn authorize_as_bound_client(
    service: &TairaAuthorityServiceV1,
    fixture: &ClientRequestFixtureV1,
    descriptors: Vec<OwnedFd>,
    now_unix_millis: u64,
) -> Result<OperationResponseV1, TairaAuthorityErrorV1> {
    let binding = service.public_binding().expect("authority binding");
    service.authorize_json(
        &fixture.request_json(),
        descriptors,
        binding.signer.client_uid,
        now_unix_millis,
    )
}

fn consumptions(state: &Path) -> BTreeMap<[u8; 32], ReplayConsumptionV1> {
    load_canonical_records(&state.join("authority-replay-consumptions-v1"))
        .expect("load public-soak replay ledger")
}

fn authorizations(state: &Path) -> BTreeMap<[u8; 32], StoredAuthorizationV1> {
    load_canonical_records(&state.join("authority-receipts-v1"))
        .expect("load public-soak authorization ledger")
}

fn mutate_scalar_preserving_shape(value: &mut Value, path: &[&str]) {
    let (leaf, parents) = path.split_last().expect("nonempty mutation path");
    let mut current = value;
    for field in parents {
        current = current
            .as_object_mut()
            .and_then(|object| object.get_mut(*field))
            .expect("signed mutation parent");
    }
    let scalar = current
        .as_object_mut()
        .and_then(|object| object.get_mut(*leaf))
        .expect("signed mutation field");
    if let Some(number) = scalar.as_u64() {
        *scalar = Value::from(number.checked_add(1).expect("signed integer mutation"));
        return;
    }
    let text = scalar.as_str().expect("signed string scalar");
    if !text.is_empty()
        && text.len().is_multiple_of(2)
        && text.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        let mut bytes = text.as_bytes().to_vec();
        bytes[0] = if bytes[0] == b'0' { b'1' } else { b'0' };
        *scalar = Value::from(String::from_utf8(bytes).expect("ASCII hex mutation"));
    } else {
        *scalar = Value::from(format!("{text}.mutated"));
    }
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "complete public-soak recovery timeline"
)]
fn public_soak_fresh_admission_retry_history_and_recovery_are_non_mutating() {
    let parent = temporary_parent();
    let observation_state = parent.path().join("public-soak-observation-state");
    let replay_state = parent.path().join("public-soak-replay-state");
    let observation = provision_observation(&observation_state);
    let observation_binding = observation.public_binding().expect("observation binding");
    let replay = provision_replay(&replay_state, observation_binding.clone());
    let replay_binding = replay.public_binding().expect("replay binding");
    assert_ne!(
        observation_binding.signer.public_key_digest,
        replay_binding.signer.public_key_digest
    );
    assert_ne!(
        observation_binding.signer.policy_digest,
        replay_binding.signer.policy_digest
    );
    assert_ne!(
        observation_binding.signer.service_uid,
        replay_binding.signer.service_uid
    );
    assert_ne!(
        observation_binding.signer.client_uid,
        replay_binding.signer.client_uid
    );
    assert_ne!(
        observation_binding.signer.administrator_uid,
        replay_binding.signer.administrator_uid
    );

    let core = valid_public_soak_subject_core();
    let completed_at = TEST_NOW_MILLIS_V1 - 1_000;
    let observation_fixture = ClientRequestFixtureV1::from_subject_and_manifest(
        TairaAuthorityRoleV1::PublicSoakObservation,
        observation_subject(&core, completed_at),
        Value::Array(Vec::new()),
    );
    assign_window(
        &observation,
        &observation_fixture,
        TEST_NOW_MILLIS_V1 - 10,
        TEST_NOW_MILLIS_V1 + 60_000,
    );
    let observation_authorized = authorize_as_bound_client(
        &observation,
        &observation_fixture,
        Vec::new(),
        TEST_NOW_MILLIS_V1,
    )
    .expect("authorize exact structural public-soak observation");
    assert_eq!(observation_authorized.status, OperationStatusV1::Ok);
    assert_eq!(result_status(&observation_authorized), "authorized");
    assert_eq!(
        sidecar_bytes(&observation_authorized, "durable_receipt"),
        b"{}\n"
    );

    let observation_verification = verification_json(&observation_fixture, &observation_authorized);
    let observation_provenance = observation.provenance().expect("observation provenance");
    let observation_consumptions = consumptions(&observation_state);
    let observation_authorizations = authorizations(&observation_state);
    let verified_observation = observation
        .verify_json(
            &observation_verification,
            Vec::new(),
            observation_binding.signer.client_uid,
        )
        .expect("historically verify observation envelope");
    assert_eq!(result_status(&verified_observation), "valid");
    assert_eq!(observation.provenance().unwrap(), observation_provenance);
    assert_eq!(consumptions(&observation_state), observation_consumptions);
    assert_eq!(
        authorizations(&observation_state),
        observation_authorizations
    );

    let authority_envelope = parse_json(&sidecar_bytes(
        &observation_authorized,
        "authority_envelope",
    ));
    let replay_subject = replay_subject(&core, completed_at, authority_envelope);
    let replay_fixture = ClientRequestFixtureV1::from_subject_and_manifest(
        TairaAuthorityRoleV1::PublicSoakReplayAdmission,
        replay_subject.clone(),
        Value::Array(Vec::new()),
    );
    assign_window(
        &replay,
        &replay_fixture,
        TEST_NOW_MILLIS_V1 - 10,
        TEST_NOW_MILLIS_V1 + 180_000,
    );
    assert!(consumptions(&replay_state).is_empty());
    assert!(authorizations(&replay_state).is_empty());

    let provenance_before_consumption = replay.provenance().expect("pre-consumption provenance");
    replay
        .inject_generic_authorization_crash_for_test(
            GenericAuthorizationCrashPhaseV1::AfterConsumptionPersistence,
        )
        .expect("inject replay-consumption crash boundary");
    assert_eq!(
        authorize_as_bound_client(&replay, &replay_fixture, Vec::new(), TEST_NOW_MILLIS_V1 + 1,),
        Err(TairaAuthorityErrorV1::State)
    );
    let crash_consumption = consumptions(&replay_state);
    assert_eq!(crash_consumption.len(), 1);
    assert_eq!(
        crash_consumption
            .get(&replay_fixture.run_id)
            .map(|record| record.operation_id),
        Some(replay_fixture.operation_id)
    );
    assert!(authorizations(&replay_state).is_empty());
    assert_eq!(replay.provenance().unwrap(), provenance_before_consumption);

    let replay_authorized =
        authorize_as_bound_client(&replay, &replay_fixture, Vec::new(), TEST_NOW_MILLIS_V1 + 1)
            .expect("fresh replay admission");
    assert_eq!(replay_authorized.status, OperationStatusV1::Ok);
    assert_eq!(result_status(&replay_authorized), "authorized");
    assert_eq!(
        sidecar_bytes(&replay_authorized, "authority_envelope"),
        sidecar_bytes(&observation_authorized, "authority_envelope"),
        "the broker must retain the exact independently signed envelope"
    );
    let consumed = consumptions(&replay_state);
    assert_eq!(consumed.len(), 1);
    assert_eq!(
        consumed.get(&replay_fixture.run_id),
        Some(&ReplayConsumptionV1 {
            run_id: replay_fixture.run_id,
            operation_id: replay_fixture.operation_id,
            request_sha256: Sha256::digest(replay_fixture.request_json()).into(),
            subject_sha256: replay_fixture.subject_sha256,
            artifact_manifest_sha256: replay_fixture.manifest_sha256,
            consumed_at_unix_millis: TEST_NOW_MILLIS_V1 + 1,
        })
    );
    assert_eq!(authorizations(&replay_state).len(), 1);

    let replay_verification = verification_json(&replay_fixture, &replay_authorized);
    let replay_provenance = replay.provenance().expect("replay provenance");
    let replay_consumptions = consumptions(&replay_state);
    let replay_authorizations = authorizations(&replay_state);
    let verified_replay = replay
        .verify_json(
            &replay_verification,
            Vec::new(),
            replay_binding.signer.client_uid,
        )
        .expect("historically verify both public-soak signatures");
    assert_eq!(result_status(&verified_replay), "valid");
    assert_eq!(replay.provenance().unwrap(), replay_provenance);
    assert_eq!(consumptions(&replay_state), replay_consumptions);
    assert_eq!(authorizations(&replay_state), replay_authorizations);

    let replay_envelope = sidecar_bytes(&replay_authorized, "authority_envelope");
    let replay_receipt = sidecar_bytes(&replay_authorized, "durable_receipt");
    let exact_retry = authorize_as_bound_client(
        &replay,
        &replay_fixture,
        Vec::new(),
        TEST_NOW_MILLIS_V1 + 60_001,
    )
    .expect("recover exact replay admission after observation expiry");
    assert_eq!(exact_retry.status, OperationStatusV1::Replayed);
    assert_eq!(
        sidecar_bytes(&exact_retry, "authority_envelope"),
        replay_envelope
    );
    assert_eq!(
        sidecar_bytes(&exact_retry, "durable_receipt"),
        replay_receipt
    );
    assert_eq!(replay.provenance().unwrap(), replay_provenance);
    assert_eq!(consumptions(&replay_state), replay_consumptions);

    let conflict_path = create_artifact(parent.path(), "replay-conflict.bin", b"conflict");
    let conflict_manifest = ClientRequestFixtureV1::new(
        TairaAuthorityRoleV1::PublicSoakReplayAdmission,
        "manifest-only-template",
        &[("replay-conflict.bin", b"conflict")],
    )
    .manifest;
    let conflicting = ClientRequestFixtureV1::from_subject_and_manifest(
        TairaAuthorityRoleV1::PublicSoakReplayAdmission,
        replay_subject,
        conflict_manifest,
    );
    assert_eq!(conflicting.run_id, replay_fixture.run_id);
    assert_ne!(conflicting.operation_id, replay_fixture.operation_id);
    assert!(matches!(
        authorize_as_bound_client(
            &replay,
            &conflicting,
            read_only_descriptors(&[&conflict_path]),
            TEST_NOW_MILLIS_V1 + 2,
        ),
        Err(TairaAuthorityErrorV1::Conflict | TairaAuthorityErrorV1::Rejected)
    ));
    assert_eq!(consumptions(&replay_state), replay_consumptions);

    drop(observation);
    drop(replay);
    let reopened_observation =
        TairaAuthorityServiceV1::open_for_test(&observation_state, observation_wrapping_key())
            .expect("reopen observation service");
    let reopened_replay =
        TairaAuthorityServiceV1::open_for_test(&replay_state, replay_wrapping_key())
            .expect("reopen replay service");
    assert_eq!(
        result_status(
            &reopened_observation
                .verify_json(
                    &observation_verification,
                    Vec::new(),
                    observation_binding.signer.client_uid,
                )
                .expect("verify observation after reopening")
        ),
        "valid"
    );
    assert_eq!(
        result_status(
            &reopened_replay
                .verify_json(
                    &replay_verification,
                    Vec::new(),
                    replay_binding.signer.client_uid,
                )
                .expect("verify replay admission after reopening")
        ),
        "valid"
    );
    assert_eq!(
        reopened_observation.provenance().unwrap(),
        observation_provenance
    );
    assert_eq!(reopened_replay.provenance().unwrap(), replay_provenance);
    assert_eq!(consumptions(&observation_state), observation_consumptions);
    assert_eq!(consumptions(&replay_state), replay_consumptions);
    assert_eq!(
        authorizations(&observation_state),
        observation_authorizations
    );
    assert_eq!(authorizations(&replay_state), replay_authorizations);
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "complete temporal substitution matrix"
)]
fn public_soak_rejects_temporal_binding_key_and_policy_substitution_before_consuming() {
    let parent = temporary_parent();
    let observation_state = parent.path().join("observation-state");
    let replay_state = parent.path().join("replay-state");
    let observation = provision_observation(&observation_state);
    let observation_binding = observation.public_binding().expect("observation binding");
    let replay = provision_replay(&replay_state, observation_binding.clone());
    let core = valid_public_soak_subject_core();
    let completed_at = TEST_NOW_MILLIS_V1 - 1_000;
    let observation_fixture = ClientRequestFixtureV1::from_subject_and_manifest(
        TairaAuthorityRoleV1::PublicSoakObservation,
        observation_subject(&core, completed_at),
        Value::Array(Vec::new()),
    );
    assign_window(
        &observation,
        &observation_fixture,
        TEST_NOW_MILLIS_V1 - 10,
        TEST_NOW_MILLIS_V1 + 60_000,
    );
    let observation_authorized = authorize_as_bound_client(
        &observation,
        &observation_fixture,
        Vec::new(),
        TEST_NOW_MILLIS_V1,
    )
    .expect("observation envelope");
    let envelope = parse_json(&sidecar_bytes(
        &observation_authorized,
        "authority_envelope",
    ));
    let envelope_expires = envelope
        .get("claims")
        .and_then(Value::as_object)
        .and_then(|claims| claims.get("expires_at_unix_ms"))
        .and_then(Value::as_u64)
        .expect("envelope expiry");

    let invalid_times = [
        (
            "completion-after-issuance",
            TEST_NOW_MILLIS_V1 + 1,
            TEST_NOW_MILLIS_V1,
        ),
        (
            "completion-too-old",
            TEST_NOW_MILLIS_V1 - PUBLIC_SOAK_MAX_AUTHORITY_LIFETIME_MILLIS_V1 - 1,
            TEST_NOW_MILLIS_V1,
        ),
        (
            "admission-before-issuance",
            completed_at + 1,
            TEST_NOW_MILLIS_V1 - 1,
        ),
        (
            "admission-after-expiry",
            completed_at + 2,
            envelope_expires + 1,
        ),
    ];
    for (case, claimed_completion, admission_time) in invalid_times {
        let fixture = ClientRequestFixtureV1::from_subject_and_manifest(
            TairaAuthorityRoleV1::PublicSoakReplayAdmission,
            replay_subject(&core, claimed_completion, envelope.clone()),
            Value::Array(Vec::new()),
        );
        assign_window(
            &replay,
            &fixture,
            TEST_NOW_MILLIS_V1 - 100,
            envelope_expires + 120_000,
        );
        let before_consumptions = consumptions(&replay_state);
        let before_provenance = replay.provenance().expect("pre-rejection provenance");
        assert!(
            authorize_as_bound_client(&replay, &fixture, Vec::new(), admission_time).is_err(),
            "invalid public-soak time was accepted: {case}"
        );
        assert_eq!(
            consumptions(&replay_state),
            before_consumptions,
            "invalid public-soak time consumed replay state: {case}"
        );
        assert_eq!(
            replay.provenance().unwrap(),
            before_provenance,
            "invalid public-soak time signed a receipt: {case}"
        );
    }

    let mut substituted_namespace = replay_subject(&core, completed_at + 3, envelope.clone());
    substituted_namespace
        .as_object_mut()
        .unwrap()
        .insert("replay_namespace".into(), Value::from("substituted-policy"));
    let substituted_namespace = ClientRequestFixtureV1::from_subject_and_manifest(
        TairaAuthorityRoleV1::PublicSoakReplayAdmission,
        substituted_namespace,
        Value::Array(Vec::new()),
    );
    assign_window(
        &replay,
        &substituted_namespace,
        TEST_NOW_MILLIS_V1 - 10,
        TEST_NOW_MILLIS_V1 + 120_000,
    );
    let before = consumptions(&replay_state);
    assert!(
        authorize_as_bound_client(
            &replay,
            &substituted_namespace,
            Vec::new(),
            TEST_NOW_MILLIS_V1 + 1,
        )
        .is_err()
    );
    assert_eq!(consumptions(&replay_state), before);

    let mut substituted_key_envelope = envelope.clone();
    mutate_scalar_preserving_shape(&mut substituted_key_envelope, &["authority_key_id"]);
    let substituted_key = ClientRequestFixtureV1::from_subject_and_manifest(
        TairaAuthorityRoleV1::PublicSoakReplayAdmission,
        replay_subject(&core, completed_at + 4, substituted_key_envelope),
        Value::Array(Vec::new()),
    );
    assign_window(
        &replay,
        &substituted_key,
        TEST_NOW_MILLIS_V1 - 10,
        TEST_NOW_MILLIS_V1 + 120_000,
    );
    let before = consumptions(&replay_state);
    assert!(
        authorize_as_bound_client(
            &replay,
            &substituted_key,
            Vec::new(),
            TEST_NOW_MILLIS_V1 + 1,
        )
        .is_err()
    );
    assert_eq!(consumptions(&replay_state), before);

    let foreign_state = parent.path().join("foreign-observation-state");
    let foreign_observation = TairaAuthorityServiceV1::provision_for_test(
        &foreign_state,
        soak_provisioning(
            TairaAuthorityRoleV1::PublicSoakObservation,
            "foreign-observation",
            301,
            302,
            303,
            0x73,
        ),
        SoftwareSignerWrappingKeyV1::try_from_bytes([0xD3; 32]).unwrap(),
    )
    .expect("foreign observation authority");
    let foreign_replay_state = parent.path().join("foreign-replay-state");
    let foreign_replay =
        TairaAuthorityServiceV1::provision_with_public_soak_observation_binding_for_test(
            &foreign_replay_state,
            soak_provisioning(
                TairaAuthorityRoleV1::PublicSoakReplayAdmission,
                "foreign-replay",
                401,
                0,
                403,
                0x74,
            ),
            SoftwareSignerWrappingKeyV1::try_from_bytes([0xD4; 32]).unwrap(),
            foreign_observation.public_binding().unwrap(),
        )
        .expect("replay authority with substituted observation binding");
    let wrong_binding_fixture = ClientRequestFixtureV1::from_subject_and_manifest(
        TairaAuthorityRoleV1::PublicSoakReplayAdmission,
        replay_subject(&core, completed_at, envelope.clone()),
        Value::Array(Vec::new()),
    );
    assign_window(
        &foreign_replay,
        &wrong_binding_fixture,
        TEST_NOW_MILLIS_V1 - 10,
        TEST_NOW_MILLIS_V1 + 120_000,
    );
    let before_provenance = foreign_replay.provenance().unwrap();
    assert!(
        authorize_as_bound_client(
            &foreign_replay,
            &wrong_binding_fixture,
            Vec::new(),
            TEST_NOW_MILLIS_V1 + 1,
        )
        .is_err()
    );
    assert!(consumptions(&foreign_replay_state).is_empty());
    assert_eq!(foreign_replay.provenance().unwrap(), before_provenance);

    let policy_fixture = ClientRequestFixtureV1::from_subject_and_manifest(
        TairaAuthorityRoleV1::PublicSoakReplayAdmission,
        replay_subject(&core, completed_at + 5, envelope),
        Value::Array(Vec::new()),
    );
    let mut substituted_policy = parse_json(&policy_fixture.assignment_json(
        &replay,
        TEST_NOW_MILLIS_V1 - 20,
        TEST_NOW_MILLIS_V1 - 10,
        TEST_NOW_MILLIS_V1 + 120_000,
    ));
    substituted_policy
        .as_object_mut()
        .unwrap()
        .insert("policy_sha256".into(), digest_value(0xFF));
    let before_consumptions = consumptions(&replay_state);
    let before_provenance = replay.provenance().expect("pre-policy-refusal provenance");
    assert!(
        replay
            .assign_run_json(
                &canonical_json_line(&substituted_policy),
                TEST_NOW_MILLIS_V1 - 10,
            )
            .is_err(),
        "substituted assignment policy was accepted"
    );
    assert_eq!(consumptions(&replay_state), before_consumptions);
    assert_eq!(replay.provenance().unwrap(), before_provenance);
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "all signed-field mutations remain explicit"
)]
fn public_soak_rejects_every_individually_mutated_signed_field() {
    let parent = temporary_parent();
    let observation_state = parent.path().join("observation-state");
    let replay_state = parent.path().join("replay-state");
    let observation = provision_observation(&observation_state);
    let observation_binding = observation.public_binding().unwrap();
    let replay = provision_replay(&replay_state, observation_binding);
    let core = valid_public_soak_subject_core();
    let completed_at = TEST_NOW_MILLIS_V1 - 1_000;
    let observation_fixture = ClientRequestFixtureV1::from_subject_and_manifest(
        TairaAuthorityRoleV1::PublicSoakObservation,
        observation_subject(&core, completed_at),
        Value::Array(Vec::new()),
    );
    assign_window(
        &observation,
        &observation_fixture,
        TEST_NOW_MILLIS_V1 - 10,
        TEST_NOW_MILLIS_V1 + 60_000,
    );
    let observation_authorized = authorize_as_bound_client(
        &observation,
        &observation_fixture,
        Vec::new(),
        TEST_NOW_MILLIS_V1,
    )
    .expect("observation authorization");
    let replay_fixture = ClientRequestFixtureV1::from_subject_and_manifest(
        TairaAuthorityRoleV1::PublicSoakReplayAdmission,
        replay_subject(
            &core,
            completed_at,
            parse_json(&sidecar_bytes(
                &observation_authorized,
                "authority_envelope",
            )),
        ),
        Value::Array(Vec::new()),
    );
    assign_window(
        &replay,
        &replay_fixture,
        TEST_NOW_MILLIS_V1 - 10,
        TEST_NOW_MILLIS_V1 + 120_000,
    );
    authorize_as_bound_client(&replay, &replay_fixture, Vec::new(), TEST_NOW_MILLIS_V1 + 1)
        .expect("replay authorization");

    let observation_stored = authorizations(&observation_state)
        .remove(&observation_fixture.operation_id)
        .expect("stored observation authorization");
    let observation_fields: &[&[&str]] = &[
        &["schema"],
        &["schema_version"],
        &["authority_key_id"],
        &["signature_algorithm"],
        &["signature"],
        &["claims", "schema"],
        &["claims", "subject_digest"],
        &["claims", "replay_namespace"],
        &["claims", "replay_id"],
        &["claims", "issued_at_unix_ms"],
        &["claims", "expires_at_unix_ms"],
    ];
    for path in observation_fields {
        let mut mutated = observation_stored.clone();
        let mut envelope = parse_json(&mutated.authority_envelope_json);
        mutate_scalar_preserving_shape(&mut envelope, path);
        mutated.authority_envelope_json = canonical_json_line(&envelope);
        assert!(
            observation
                .verify_stored_authorization_for_test(&mutated)
                .is_err(),
            "mutated observation envelope field was accepted: {path:?}"
        );
    }

    let replay_stored = authorizations(&replay_state)
        .remove(&replay_fixture.operation_id)
        .expect("stored replay authorization");
    for path in observation_fields {
        let mut mutated = replay_stored.clone();
        let mut envelope = parse_json(&mutated.authority_envelope_json);
        mutate_scalar_preserving_shape(&mut envelope, path);
        mutated.authority_envelope_json = canonical_json_line(&envelope);
        assert!(
            replay
                .verify_stored_authorization_for_test(&mutated)
                .is_err(),
            "mutated broker-retained observation field was accepted: {path:?}"
        );
    }

    let durable_receipt_fields: &[&[&str]] = &[
        &["schema"],
        &["schema_version"],
        &["broker_key_id"],
        &["signature_algorithm"],
        &["signature"],
        &["claims", "schema"],
        &["claims", "decision"],
        &["claims", "receipt_id"],
        &["claims", "subject_digest"],
        &["claims", "authority_envelope_sha256"],
        &["claims", "authority_key_id"],
        &["claims", "replay_namespace"],
        &["claims", "replay_id"],
        &["claims", "admitted_at_unix_ms"],
    ];
    for path in durable_receipt_fields {
        let mut mutated = replay_stored.clone();
        let mut receipt = parse_json(&mutated.durable_receipt_json);
        mutate_scalar_preserving_shape(&mut receipt, path);
        mutated.durable_receipt_json = canonical_json_line(&receipt);
        assert!(
            replay
                .verify_stored_authorization_for_test(&mutated)
                .is_err(),
            "mutated durable admission receipt field was accepted: {path:?}"
        );
    }
}
