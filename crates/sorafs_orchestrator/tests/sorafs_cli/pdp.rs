use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use iroha_crypto::PublicKey;
use iroha_data_model::NetworkId;
use sha2::Sha256;
use sorafs_manifest::{PdpChallengeV1, PdpCommitmentV1, PdpProofV1};
const PDP_OPERATOR_SIGNATURE_DOMAIN: &[u8] = b"iroha.operator.http-request.network.v1\0";
fn pdp_fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("fixtures/sorafs_manifest/pdp")
        .join(name)
}
fn pdp_object<const N: usize>(entries: [(&str, Value); N]) -> Value {
    Value::Object(
        entries
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    )
}
fn pdp_operator_key(directory: &Path, algorithm: Algorithm) -> (PathBuf, String) {
    let key_pair = KeyPair::try_from_seed(
        format!("sorafs-cli-pdp-operator-{algorithm:?}").into_bytes(),
        algorithm,
    )
    .expect("derive PDP operator test key");
    let path = directory.join(format!("pdp-operator-{algorithm:?}.key"));
    let exposed = ExposedPrivateKey(key_pair.private_key().clone()).to_string();
    fs::write(&path, format!("{exposed}\n")).expect("write PDP operator test key");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
            .expect("secure PDP operator test key");
    }
    (path, key_pair.public_key().to_string())
}
fn pdp_common_args(server: &MockServer, key_path: &Path) -> Vec<String> {
    vec![
        format!("--torii-url={}", server.base_url()),
        format!("--network-id={TEST_NETWORK_ID_LITERAL}"),
        format!("--operator-private-key-file={}", key_path.display()),
    ]
}
fn pdp_assert(
    operation: &str,
    common: &[String],
    operation_args: &[String],
) -> assert_cmd::assert::Assert {
    let mut command = sorafs_cli_cmd();
    command
        .arg("pdp")
        .arg(operation)
        .args(common)
        .args(operation_args);
    command.assert()
}
fn pdp_single_header(request: &HttpMockRequest, name: &str) -> Option<String> {
    let mut values = request
        .headers_vec()
        .iter()
        .filter(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.clone());
    let value = values.next()?;
    values.next().is_none().then_some(value)
}
fn pdp_signed_request_matches(
    request: &HttpMockRequest,
    expected_path: &str,
    expected_body: &[u8],
    expected_public_key: &str,
) -> bool {
    let uri = request.uri();
    if request.method_str() != "POST"
        || uri.path() != expected_path
        || uri.query().is_some()
        || request.body_ref() != expected_body
        || pdp_single_header(request, "content-type").as_deref() != Some("application/json")
        || pdp_single_header(request, "accept").as_deref() != Some("application/json")
        || pdp_single_header(request, "accept-encoding").as_deref() != Some("identity")
    {
        return false;
    }
    let public_key_text = match pdp_single_header(request, "x-iroha-operator-public-key") {
        Some(value) if value == expected_public_key => value,
        _ => return false,
    };
    let public_key = match public_key_text.parse::<PublicKey>() {
        Ok(value) if value.try_algorithm() == Ok(Algorithm::Ed25519) => value,
        _ => return false,
    };
    let timestamp_text = match pdp_single_header(request, "x-iroha-operator-timestamp-ms") {
        Some(value) => value,
        None => return false,
    };
    let timestamp_ms = match timestamp_text.parse::<u64>() {
        Ok(value) if value != 0 && value.to_string() == timestamp_text => value,
        _ => return false,
    };
    let nonce = match pdp_single_header(request, "x-iroha-operator-nonce") {
        Some(value) => value,
        None => return false,
    };
    match URL_SAFE_NO_PAD.decode(nonce.as_bytes()) {
        Ok(bytes) if bytes.len() == 12 && URL_SAFE_NO_PAD.encode(bytes) == nonce => {}
        _ => return false,
    }
    let signature_text = match pdp_single_header(request, "x-iroha-operator-signature") {
        Some(value) => value,
        None => return false,
    };
    let signature_bytes = match BASE64_STANDARD.decode(signature_text.as_bytes()) {
        Ok(bytes) if BASE64_STANDARD.encode(&bytes) == signature_text => bytes,
        _ => return false,
    };
    let signature = match iroha_crypto::ed25519_parse_signature(&signature_bytes) {
        Ok(value) => value,
        Err(_) => return false,
    };
    let network_id = match TEST_NETWORK_ID_LITERAL.parse::<NetworkId>() {
        Ok(value) => value,
        Err(_) => return false,
    };
    let canonical_request = format!(
        "POST\n{expected_path}\n\n{}",
        hex_encode(Sha256::digest(request.body_ref()))
    );
    let mut message = Vec::new();
    message.extend_from_slice(PDP_OPERATOR_SIGNATURE_DOMAIN);
    message.extend_from_slice(network_id.as_bytes());
    message.extend_from_slice(canonical_request.as_bytes());
    message.push(b'\n');
    message.extend_from_slice(timestamp_ms.to_string().as_bytes());
    message.push(b'\n');
    message.extend_from_slice(nonce.as_bytes());
    signature.verify(&public_key, &message).is_ok()
}
fn pdp_json_mock<'a>(
    server: &'a MockServer,
    path: &str,
    expected_body: &Value,
    expected_public_key: &str,
    content_type: &str,
    response: &Value,
) -> Mock<'a> {
    let path = path.to_owned();
    let matcher_path = path.clone();
    let expected_body = to_vec(expected_body).expect("encode expected PDP request");
    let expected_public_key = expected_public_key.to_owned();
    let response = to_vec(response).expect("encode PDP response fixture");
    let response_len = response.len().to_string();
    let content_type = content_type.to_owned();
    server.mock(move |when, then| {
        when.method(POST).path(path).is_true(move |request| {
            pdp_signed_request_matches(request, &matcher_path, &expected_body, &expected_public_key)
        });
        then.status(200)
            .header("Content-Type", content_type)
            .header("Content-Length", response_len)
            .body(response);
    })
}
#[cfg(any(
    target_os = "android",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos"
))]
fn pdp_no_content_mock<'a>(
    server: &'a MockServer,
    expected_body: &Value,
    expected_public_key: &str,
) -> Mock<'a> {
    let expected_body = to_vec(expected_body).expect("encode expected PDP next request");
    let expected_public_key = expected_public_key.to_owned();
    server.mock(move |when, then| {
        when.method(POST)
            .path("/v1/sorafs/pdp/next")
            .is_true(move |request| {
                pdp_signed_request_matches(
                    request,
                    "/v1/sorafs/pdp/next",
                    &expected_body,
                    &expected_public_key,
                )
            });
        then.status(204);
    })
}
#[cfg(any(
    target_os = "android",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos"
))]
fn pdp_pending_status(challenge: &PdpChallengeV1) -> Value {
    pdp_object([
        ("sequence", Value::from(1_u64)),
        (
            "challenge_id_hex",
            Value::from(hex_encode(challenge.challenge_id)),
        ),
        (
            "manifest_digest_hex",
            Value::from(hex_encode(challenge.manifest_digest)),
        ),
        (
            "provider_id_hex",
            Value::from(hex_encode(challenge.provider_id)),
        ),
        ("epoch_id", Value::from(challenge.epoch_id)),
        ("lifecycle", Value::from("pending")),
        (
            "response_deadline_unix",
            Value::from(challenge.response_deadline_unix),
        ),
    ])
}
fn pdp_terminal_status(challenge: &PdpChallengeV1, proof: &PdpProofV1) -> Value {
    pdp_object([
        ("sequence", Value::from(1_u64)),
        (
            "challenge_id_hex",
            Value::from(hex_encode(challenge.challenge_id)),
        ),
        (
            "manifest_digest_hex",
            Value::from(hex_encode(challenge.manifest_digest)),
        ),
        (
            "provider_id_hex",
            Value::from(hex_encode(challenge.provider_id)),
        ),
        ("epoch_id", Value::from(challenge.epoch_id)),
        ("lifecycle", Value::from("terminal")),
        ("decision", Value::from("accepted")),
        (
            "proof_digest_hex",
            Value::from(hex_encode(
                proof.proof_digest().expect("derive PDP proof digest"),
            )),
        ),
    ])
}
#[cfg(any(
    target_os = "android",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos"
))]
#[test]
fn pdp_operator_commands_sign_exact_bodies_and_validate_all_five_routes() {
    let directory = tempdir().expect("create canonical PDP CLI tempdir");
    let server = MockServer::start();
    let (key_path, public_key) = pdp_operator_key(directory.path(), Algorithm::Ed25519);
    let common = pdp_common_args(&server, &key_path);
    let commitment_path = pdp_fixture("commitment_v1.to");
    let challenge_path = pdp_fixture("challenge_v1.to");
    let proof_path = pdp_fixture("proof_v1.to");
    let commitment_bytes = fs::read(&commitment_path).expect("read PDP commitment fixture");
    let challenge_bytes = fs::read(&challenge_path).expect("read PDP challenge fixture");
    let proof_bytes = fs::read(&proof_path).expect("read PDP proof fixture");
    let challenge: PdpChallengeV1 =
        decode_from_bytes(&challenge_bytes).expect("decode PDP challenge fixture");
    let proof: PdpProofV1 = decode_from_bytes(&proof_bytes).expect("decode PDP proof fixture");
    let challenge_id_hex = hex_encode(challenge.challenge_id);
    let provider_id_hex = hex_encode(challenge.provider_id);
    let enqueued_before_issue = challenge
        .issued_at_unix
        .checked_sub(1)
        .filter(|timestamp| *timestamp != 0)
        .expect("PDP fixture permits enqueue before its future-skewed issue time");
    let enqueue_request = pdp_object([
        (
            "commitment_b64",
            Value::from(BASE64_STANDARD.encode(&commitment_bytes)),
        ),
        (
            "challenge_b64",
            Value::from(BASE64_STANDARD.encode(&challenge_bytes)),
        ),
        ("expected_epoch_id", Value::from(challenge.epoch_id)),
    ]);
    let enqueue_response = pdp_object([
        ("result", Value::from("inserted")),
        ("sequence", Value::from(1_u64)),
        ("challenge_id_hex", Value::from(challenge_id_hex.clone())),
    ]);
    let enqueue_mock = pdp_json_mock(
        &server,
        "/v1/sorafs/pdp/challenge",
        &enqueue_request,
        &public_key,
        "application/json",
        &enqueue_response,
    );
    let next_request = pdp_object([("provider_id_hex", Value::from(provider_id_hex.clone()))]);
    let next_response = pdp_object([
        ("sequence", Value::from(1_u64)),
        ("challenge_id_hex", Value::from(challenge_id_hex.clone())),
        (
            "challenge_b64",
            Value::from(BASE64_STANDARD.encode(&challenge_bytes)),
        ),
        ("enqueued_at_unix", Value::from(enqueued_before_issue)),
    ]);
    let next_mock = pdp_json_mock(
        &server,
        "/v1/sorafs/pdp/next",
        &next_request,
        &public_key,
        "application/json",
        &next_response,
    );
    let submit_request = pdp_object([
        ("challenge_id_hex", Value::from(challenge_id_hex.clone())),
        (
            "proof_b64",
            Value::from(BASE64_STANDARD.encode(&proof_bytes)),
        ),
    ]);
    let terminal_status = pdp_terminal_status(&challenge, &proof);
    let submit_mock = pdp_json_mock(
        &server,
        "/v1/sorafs/pdp/proof",
        &submit_request,
        &public_key,
        "application/json",
        &terminal_status,
    );
    let status_request = pdp_object([("challenge_id_hex", Value::from(challenge_id_hex.clone()))]);
    let pending_status = pdp_pending_status(&challenge);
    let status_mock = pdp_json_mock(
        &server,
        "/v1/sorafs/pdp/status",
        &status_request,
        &public_key,
        "application/json",
        &pending_status,
    );
    let export_request = pdp_object([
        ("after_sequence", Value::from(0_u64)),
        ("limit", Value::from(2_u32)),
    ]);
    let export_response = pdp_object([
        ("items", Value::Array(vec![pending_status.clone()])),
        ("next_sequence", Value::from(1_u64)),
    ]);
    let export_mock = pdp_json_mock(
        &server,
        "/v1/sorafs/pdp/export",
        &export_request,
        &public_key,
        "application/json",
        &export_response,
    );
    let enqueue_args = vec![
        format!("--commitment={}", commitment_path.display()),
        format!("--challenge={}", challenge_path.display()),
        format!("--expected-epoch-id={}", challenge.epoch_id),
    ];
    let enqueue = pdp_assert("enqueue", &common, &enqueue_args).success();
    assert_eq!(
        from_slice::<Value>(&enqueue.get_output().stdout).expect("parse enqueue stdout"),
        enqueue_response
    );
    let challenge_out = directory.path().join("next-challenge.to");
    let next_args = vec![
        format!("--provider-id-hex={provider_id_hex}"),
        format!("--challenge-out={}", challenge_out.display()),
    ];
    let next = pdp_assert("next", &common, &next_args).success();
    let next_summary: Value = from_slice(&next.get_output().stdout).expect("parse PDP next stdout");
    assert_eq!(
        next_summary.get("result").and_then(Value::as_str),
        Some("challenge")
    );
    assert_eq!(
        fs::read(&challenge_out).expect("read PDP next output"),
        challenge_bytes
    );
    let submit_args = vec![
        format!("--challenge-id-hex={challenge_id_hex}"),
        format!("--proof={}", proof_path.display()),
    ];
    let submit = pdp_assert("submit", &common, &submit_args).success();
    assert_eq!(
        from_slice::<Value>(&submit.get_output().stdout).expect("parse submit stdout"),
        terminal_status
    );
    let status_args = vec![format!("--challenge-id-hex={challenge_id_hex}")];
    let status = pdp_assert("status", &common, &status_args).success();
    assert_eq!(
        from_slice::<Value>(&status.get_output().stdout).expect("parse status stdout"),
        pending_status
    );
    let export_out = directory.path().join("pdp-export.json");
    let export_args = vec![
        "--after-sequence=0".to_owned(),
        "--limit=2".to_owned(),
        format!("--out={}", export_out.display()),
    ];
    let export = pdp_assert("export", &common, &export_args).success();
    let export_summary: Value =
        from_slice(&export.get_output().stdout).expect("parse export stdout");
    assert_eq!(
        export_summary.get("item_count").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        from_slice::<Value>(&fs::read(&export_out).expect("read PDP export"))
            .expect("parse PDP export"),
        export_response
    );
    enqueue_mock.assert_calls(1);
    next_mock.assert_calls(1);
    submit_mock.assert_calls(1);
    status_mock.assert_calls(1);
    export_mock.assert_calls(1);
}
#[cfg(any(
    target_os = "android",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos"
))]
#[test]
fn pdp_next_accepts_exact_no_content_without_creating_output() {
    let directory = tempdir().expect("create PDP no-content tempdir");
    let server = MockServer::start();
    let (key_path, public_key) = pdp_operator_key(directory.path(), Algorithm::Ed25519);
    let common = pdp_common_args(&server, &key_path);
    let challenge_bytes = fs::read(pdp_fixture("challenge_v1.to")).expect("read challenge");
    let challenge: PdpChallengeV1 = decode_from_bytes(&challenge_bytes).expect("decode challenge");
    let provider_id_hex = hex_encode(challenge.provider_id);
    let request = pdp_object([("provider_id_hex", Value::from(provider_id_hex.clone()))]);
    let mock = pdp_no_content_mock(&server, &request, &public_key);
    let output = directory.path().join("must-not-exist.to");
    let args = vec![
        format!("--provider-id-hex={provider_id_hex}"),
        format!("--challenge-out={}", output.display()),
    ];
    let assert = pdp_assert("next", &common, &args).success();
    let summary: Value = from_slice(&assert.get_output().stdout).expect("parse next empty stdout");
    assert_eq!(summary.get("result").and_then(Value::as_str), Some("empty"));
    assert!(!output.exists());
    mock.assert_calls(1);
}
#[cfg(any(
    target_os = "android",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos"
))]
#[test]
fn pdp_next_rejects_unknown_response_fields_before_output() {
    let directory = tempdir().expect("create PDP unknown-field tempdir");
    let server = MockServer::start();
    let (key_path, public_key) = pdp_operator_key(directory.path(), Algorithm::Ed25519);
    let common = pdp_common_args(&server, &key_path);
    let challenge_bytes = fs::read(pdp_fixture("challenge_v1.to")).expect("read challenge");
    let challenge: PdpChallengeV1 = decode_from_bytes(&challenge_bytes).expect("decode challenge");
    let provider_id_hex = hex_encode(challenge.provider_id);
    let request = pdp_object([("provider_id_hex", Value::from(provider_id_hex.clone()))]);
    let response = pdp_object([
        ("sequence", Value::from(1_u64)),
        (
            "challenge_id_hex",
            Value::from(hex_encode(challenge.challenge_id)),
        ),
        (
            "challenge_b64",
            Value::from(BASE64_STANDARD.encode(challenge_bytes)),
        ),
        ("enqueued_at_unix", Value::from(challenge.issued_at_unix)),
        ("unexpected", Value::from(true)),
    ]);
    let mock = pdp_json_mock(
        &server,
        "/v1/sorafs/pdp/next",
        &request,
        &public_key,
        "application/json",
        &response,
    );
    let output = directory.path().join("unknown-field.to");
    let args = vec![
        format!("--provider-id-hex={provider_id_hex}"),
        format!("--challenge-out={}", output.display()),
    ];
    let assert = pdp_assert("next", &common, &args).failure();
    assert!(String::from_utf8_lossy(&assert.get_output().stderr).contains("unknown field"));
    assert!(!output.exists());
    mock.assert_calls(1);
}
#[cfg(any(
    target_os = "android",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos"
))]
#[test]
fn pdp_next_rejects_enqueued_timestamp_after_challenge_deadline() {
    let directory = tempdir().expect("create PDP next-timestamp tempdir");
    let server = MockServer::start();
    let (key_path, public_key) = pdp_operator_key(directory.path(), Algorithm::Ed25519);
    let common = pdp_common_args(&server, &key_path);
    let challenge_bytes = fs::read(pdp_fixture("challenge_v1.to")).expect("read challenge");
    let challenge: PdpChallengeV1 = decode_from_bytes(&challenge_bytes).expect("decode challenge");
    let provider_id_hex = hex_encode(challenge.provider_id);
    let request = pdp_object([("provider_id_hex", Value::from(provider_id_hex.clone()))]);
    let response = pdp_object([
        ("sequence", Value::from(1_u64)),
        (
            "challenge_id_hex",
            Value::from(hex_encode(challenge.challenge_id)),
        ),
        (
            "challenge_b64",
            Value::from(BASE64_STANDARD.encode(challenge_bytes)),
        ),
        (
            "enqueued_at_unix",
            Value::from(challenge.response_deadline_unix + 1),
        ),
    ]);
    let mock = pdp_json_mock(
        &server,
        "/v1/sorafs/pdp/next",
        &request,
        &public_key,
        "application/json",
        &response,
    );
    let output = directory.path().join("late-enqueue.to");
    let args = vec![
        format!("--provider-id-hex={provider_id_hex}"),
        format!("--challenge-out={}", output.display()),
    ];
    let assert = pdp_assert("next", &common, &args).failure();
    assert!(String::from_utf8_lossy(&assert.get_output().stderr).contains("binding"));
    assert!(!output.exists());
    mock.assert_calls(1);
}
#[cfg(any(
    target_os = "android",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos"
))]
#[test]
fn pdp_export_rejects_noncanonical_content_type_before_output() {
    let directory = tempdir().expect("create PDP content-type tempdir");
    let server = MockServer::start();
    let (key_path, public_key) = pdp_operator_key(directory.path(), Algorithm::Ed25519);
    let common = pdp_common_args(&server, &key_path);
    let request = pdp_object([
        ("after_sequence", Value::from(0_u64)),
        ("limit", Value::from(100_u32)),
    ]);
    let response = pdp_object([
        ("items", Value::Array(Vec::new())),
        ("next_sequence", Value::from(0_u64)),
    ]);
    let mock = pdp_json_mock(
        &server,
        "/v1/sorafs/pdp/export",
        &request,
        &public_key,
        "application/json; charset=utf-8",
        &response,
    );
    let output = directory.path().join("wrong-content-type.json");
    let args = vec![format!("--out={}", output.display())];
    let assert = pdp_assert("export", &common, &args).failure();
    assert!(
        String::from_utf8_lossy(&assert.get_output().stderr).contains("canonical Content-Type")
    );
    assert!(!output.exists());
    mock.assert_calls(1);
}
#[cfg(any(
    target_os = "android",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos"
))]
#[test]
fn pdp_outputs_are_create_new_and_fail_before_network() {
    let directory = tempdir().expect("create PDP no-clobber tempdir");
    let server = MockServer::start();
    let (key_path, _) = pdp_operator_key(directory.path(), Algorithm::Ed25519);
    let common = pdp_common_args(&server, &key_path);
    let challenge_bytes = fs::read(pdp_fixture("challenge_v1.to")).expect("read challenge");
    let challenge: PdpChallengeV1 = decode_from_bytes(&challenge_bytes).expect("decode challenge");
    let provider_id_hex = hex_encode(challenge.provider_id);
    let route = server.mock(|when, then| {
        when.method(POST).path("/v1/sorafs/pdp/next");
        then.status(500);
    });
    let output = directory.path().join("existing-challenge.to");
    fs::write(&output, b"keep-me").expect("write existing PDP output");
    let args = vec![
        format!("--provider-id-hex={provider_id_hex}"),
        format!("--challenge-out={}", output.display()),
    ];
    let assert = pdp_assert("next", &common, &args).failure();
    assert!(String::from_utf8_lossy(&assert.get_output().stderr).contains("never clobber"));
    assert_eq!(
        fs::read(&output).expect("read preserved output"),
        b"keep-me"
    );
    route.assert_calls(0);
}
#[cfg(any(
    target_os = "android",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos"
))]
#[test]
fn pdp_rejects_symlink_inputs_and_outputs_before_network() {
    use std::os::unix::fs::symlink;
    let directory = tempdir().expect("create PDP symlink tempdir");
    let server = MockServer::start();
    let (key_path, _) = pdp_operator_key(directory.path(), Algorithm::Ed25519);
    let common = pdp_common_args(&server, &key_path);
    let challenge_path = pdp_fixture("challenge_v1.to");
    let challenge_bytes = fs::read(&challenge_path).expect("read challenge");
    let challenge: PdpChallengeV1 = decode_from_bytes(&challenge_bytes).expect("decode challenge");
    let provider_id_hex = hex_encode(challenge.provider_id);
    let route = server.mock(|when, then| {
        when.method(POST);
        then.status(500);
    });
    let output_target = directory.path().join("output-target.to");
    fs::write(&output_target, b"target").expect("write symlink target");
    let output_link = directory.path().join("output-link.to");
    symlink(&output_target, &output_link).expect("create output symlink");
    let next_args = vec![
        format!("--provider-id-hex={provider_id_hex}"),
        format!("--challenge-out={}", output_link.display()),
    ];
    let next_assert = pdp_assert("next", &common, &next_args).failure();
    assert!(
        String::from_utf8_lossy(&next_assert.get_output().stderr).contains("must not be a symlink")
    );
    assert_eq!(fs::read(&output_target).expect("read target"), b"target");
    let commitment_link = directory.path().join("commitment-link.to");
    symlink(pdp_fixture("commitment_v1.to"), &commitment_link).expect("create commitment symlink");
    let enqueue_args = vec![
        format!("--commitment={}", commitment_link.display()),
        format!("--challenge={}", challenge_path.display()),
        format!("--expected-epoch-id={}", challenge.epoch_id),
    ];
    let enqueue_assert = pdp_assert("enqueue", &common, &enqueue_args).failure();
    assert!(
        String::from_utf8_lossy(&enqueue_assert.get_output().stderr)
            .contains("regular non-symlink")
    );
    route.assert_calls(0);
}
#[test]
fn pdp_operator_auth_rejects_non_ed25519_keys_before_network() {
    let directory = tempdir().expect("create PDP key algorithm tempdir");
    let server = MockServer::start();
    let (key_path, _) = pdp_operator_key(directory.path(), Algorithm::Secp256k1);
    let common = pdp_common_args(&server, &key_path);
    let challenge_bytes = fs::read(pdp_fixture("challenge_v1.to")).expect("read challenge");
    let challenge: PdpChallengeV1 = decode_from_bytes(&challenge_bytes).expect("decode challenge");
    let challenge_id_hex = hex_encode(challenge.challenge_id);
    let route = server.mock(|when, then| {
        when.method(POST).path("/v1/sorafs/pdp/status");
        then.status(500);
    });
    let args = vec![format!("--challenge-id-hex={challenge_id_hex}")];
    let assert = pdp_assert("status", &common, &args).failure();
    assert!(
        String::from_utf8_lossy(&assert.get_output().stderr)
            .contains("requires an Ed25519 private key")
    );
    route.assert_calls(0);
}
#[test]
fn pdp_submit_rejects_canonical_proof_with_tampered_signed_payload_before_network() {
    let directory = tempdir().expect("create PDP tampered-proof tempdir");
    let server = MockServer::start();
    let (key_path, _) = pdp_operator_key(directory.path(), Algorithm::Ed25519);
    let common = pdp_common_args(&server, &key_path);
    let proof_bytes = fs::read(pdp_fixture("proof_v1.to")).expect("read PDP proof fixture");
    let mut proof: PdpProofV1 = decode_from_bytes(&proof_bytes).expect("decode PDP proof fixture");
    proof.issued_at_unix = proof
        .issued_at_unix
        .checked_add(1)
        .expect("fixture timestamp does not overflow");
    let challenge_id_hex = hex_encode(proof.challenge_id);
    let tampered_path = directory.path().join("tampered-proof.to");
    fs::write(
        &tampered_path,
        to_bytes(&proof).expect("canonically encode tampered proof"),
    )
    .expect("write tampered proof");
    let route = server.mock(|when, then| {
        when.method(POST).path("/v1/sorafs/pdp/proof");
        then.status(500);
    });
    let args = vec![
        format!("--challenge-id-hex={challenge_id_hex}"),
        format!("--proof={}", tampered_path.display()),
    ];
    let assert = pdp_assert("submit", &common, &args).failure();
    assert!(
        String::from_utf8_lossy(&assert.get_output().stderr)
            .contains("proof signature verification failed")
    );
    route.assert_calls(0);
}
#[test]
fn pdp_enqueue_rejects_canonical_commitment_window_and_sealing_mismatches_before_network() {
    let directory = tempdir().expect("create PDP binding-mismatch tempdir");
    let server = MockServer::start();
    let (key_path, _) = pdp_operator_key(directory.path(), Algorithm::Ed25519);
    let common = pdp_common_args(&server, &key_path);
    let commitment_bytes =
        fs::read(pdp_fixture("commitment_v1.to")).expect("read PDP commitment fixture");
    let challenge_bytes =
        fs::read(pdp_fixture("challenge_v1.to")).expect("read PDP challenge fixture");
    let base_commitment: PdpCommitmentV1 =
        decode_from_bytes(&commitment_bytes).expect("decode PDP commitment fixture");
    let base_challenge: PdpChallengeV1 =
        decode_from_bytes(&challenge_bytes).expect("decode PDP challenge fixture");
    assert!(base_challenge.samples.len() > 1);
    let route = server.mock(|when, then| {
        when.method(POST).path("/v1/sorafs/pdp/challenge");
        then.status(500);
    });
    for (case, mut commitment, mut challenge) in [
        (
            "sample-window",
            base_commitment.clone(),
            base_challenge.clone(),
        ),
        ("sealed-after-issue", base_commitment, base_challenge),
    ] {
        if case == "sample-window" {
            commitment.sample_window = 1;
        } else {
            commitment.sealed_at = challenge
                .issued_at_unix
                .checked_add(1)
                .expect("fixture issue time does not overflow");
        }
        challenge.commitment_digest = commitment
            .commitment_digest()
            .expect("derive mutated commitment digest");
        challenge.challenge_id = challenge
            .derived_challenge_id()
            .expect("derive rebound challenge ID");
        commitment
            .validate()
            .expect("mutated commitment remains valid");
        challenge
            .validate()
            .expect("rebound challenge remains valid");
        let commitment_path = directory.path().join(format!("{case}-commitment.to"));
        let challenge_path = directory.path().join(format!("{case}-challenge.to"));
        fs::write(
            &commitment_path,
            to_bytes(&commitment).expect("encode mutated commitment"),
        )
        .expect("write mutated commitment");
        fs::write(
            &challenge_path,
            to_bytes(&challenge).expect("encode rebound challenge"),
        )
        .expect("write rebound challenge");
        let args = vec![
            format!("--commitment={}", commitment_path.display()),
            format!("--challenge={}", challenge_path.display()),
            format!("--expected-epoch-id={}", challenge.epoch_id),
        ];
        let assert = pdp_assert("enqueue", &common, &args).failure();
        assert!(
            String::from_utf8_lossy(&assert.get_output().stderr)
                .contains("not bound to the supplied commitment")
        );
    }
    route.assert_calls(0);
}
#[test]
fn pdp_submit_rejects_mismatched_response_proof_digest() {
    let directory = tempdir().expect("create PDP response-binding tempdir");
    let server = MockServer::start();
    let (key_path, public_key) = pdp_operator_key(directory.path(), Algorithm::Ed25519);
    let common = pdp_common_args(&server, &key_path);
    let challenge_bytes = fs::read(pdp_fixture("challenge_v1.to")).expect("read challenge");
    let proof_bytes = fs::read(pdp_fixture("proof_v1.to")).expect("read proof");
    let challenge: PdpChallengeV1 = decode_from_bytes(&challenge_bytes).expect("decode challenge");
    let proof: PdpProofV1 = decode_from_bytes(&proof_bytes).expect("decode proof");
    let challenge_id_hex = hex_encode(challenge.challenge_id);
    let request = pdp_object([
        ("challenge_id_hex", Value::from(challenge_id_hex.clone())),
        (
            "proof_b64",
            Value::from(BASE64_STANDARD.encode(&proof_bytes)),
        ),
    ]);
    let mut response = pdp_terminal_status(&challenge, &proof);
    response
        .as_object_mut()
        .expect("terminal status object")
        .insert("proof_digest_hex".into(), Value::from("99".repeat(32)));
    let mock = pdp_json_mock(
        &server,
        "/v1/sorafs/pdp/proof",
        &request,
        &public_key,
        "application/json",
        &response,
    );
    let args = vec![
        format!("--challenge-id-hex={challenge_id_hex}"),
        format!("--proof={}", pdp_fixture("proof_v1.to").display()),
    ];
    let assert = pdp_assert("submit", &common, &args).failure();
    assert!(
        String::from_utf8_lossy(&assert.get_output().stderr)
            .contains("proof digest does not match the submission")
    );
    mock.assert_calls(1);
}
#[test]
fn pdp_submit_rejects_accepted_response_with_mismatched_proof_scope() {
    let directory = tempdir().expect("create PDP proof-scope tempdir");
    let (key_path, public_key) = pdp_operator_key(directory.path(), Algorithm::Ed25519);
    let challenge_bytes = fs::read(pdp_fixture("challenge_v1.to")).expect("read challenge");
    let proof_bytes = fs::read(pdp_fixture("proof_v1.to")).expect("read proof");
    let challenge: PdpChallengeV1 = decode_from_bytes(&challenge_bytes).expect("decode challenge");
    let proof: PdpProofV1 = decode_from_bytes(&proof_bytes).expect("decode proof");
    let challenge_id_hex = hex_encode(challenge.challenge_id);
    let request = pdp_object([
        ("challenge_id_hex", Value::from(challenge_id_hex.clone())),
        (
            "proof_b64",
            Value::from(BASE64_STANDARD.encode(&proof_bytes)),
        ),
    ]);
    for (field, wrong_value) in [
        ("manifest_digest_hex", Value::from("88".repeat(32))),
        ("provider_id_hex", Value::from("77".repeat(32))),
        ("epoch_id", Value::from(proof.epoch_id + 1)),
    ] {
        let server = MockServer::start();
        let common = pdp_common_args(&server, &key_path);
        let mut response = pdp_terminal_status(&challenge, &proof);
        response
            .as_object_mut()
            .expect("terminal status object")
            .insert(field.into(), wrong_value);
        let mock = pdp_json_mock(
            &server,
            "/v1/sorafs/pdp/proof",
            &request,
            &public_key,
            "application/json",
            &response,
        );
        let args = vec![
            format!("--challenge-id-hex={challenge_id_hex}"),
            format!("--proof={}", pdp_fixture("proof_v1.to").display()),
        ];
        let assert = pdp_assert("submit", &common, &args).failure();
        assert!(
            String::from_utf8_lossy(&assert.get_output().stderr)
                .contains("does not match the submitted proof scope")
        );
        mock.assert_calls(1);
    }
}
#[test]
fn pdp_submit_rejects_terminal_reasons_that_cannot_result_from_proof_submission() {
    let directory = tempdir().expect("create PDP submit-matrix tempdir");
    let (key_path, public_key) = pdp_operator_key(directory.path(), Algorithm::Ed25519);
    let challenge_bytes = fs::read(pdp_fixture("challenge_v1.to")).expect("read challenge");
    let proof_bytes = fs::read(pdp_fixture("proof_v1.to")).expect("read proof");
    let challenge: PdpChallengeV1 = decode_from_bytes(&challenge_bytes).expect("decode challenge");
    let proof: PdpProofV1 = decode_from_bytes(&proof_bytes).expect("decode proof");
    let challenge_id_hex = hex_encode(challenge.challenge_id);
    let request = pdp_object([
        ("challenge_id_hex", Value::from(challenge_id_hex.clone())),
        (
            "proof_b64",
            Value::from(BASE64_STANDARD.encode(&proof_bytes)),
        ),
    ]);
    for rejection_reason in [
        "deadline_expired",
        "admission_inactive",
        "storage_unavailable",
    ] {
        let server = MockServer::start();
        let common = pdp_common_args(&server, &key_path);
        let mut response = pdp_terminal_status(&challenge, &proof);
        let response = response.as_object_mut().expect("terminal status object");
        response.insert("decision".into(), Value::from("rejected"));
        response.insert("rejection_reason".into(), Value::from(rejection_reason));
        response.remove("proof_digest_hex");
        let response = Value::Object(response.clone());
        let mock = pdp_json_mock(
            &server,
            "/v1/sorafs/pdp/proof",
            &request,
            &public_key,
            "application/json",
            &response,
        );
        let args = vec![
            format!("--challenge-id-hex={challenge_id_hex}"),
            format!("--proof={}", pdp_fixture("proof_v1.to").display()),
        ];
        let assert = pdp_assert("submit", &common, &args).failure();
        assert!(
            String::from_utf8_lossy(&assert.get_output().stderr)
                .contains("proof submission response has an invalid terminal outcome")
        );
        mock.assert_calls(1);
    }
}
#[cfg(any(
    target_os = "android",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos"
))]
#[test]
fn pdp_export_rejects_duplicate_challenge_ids_and_provider_scopes() {
    let directory = tempdir().expect("create PDP duplicate-export tempdir");
    let (key_path, public_key) = pdp_operator_key(directory.path(), Algorithm::Ed25519);
    let challenge_bytes = fs::read(pdp_fixture("challenge_v1.to")).expect("read challenge");
    let challenge: PdpChallengeV1 = decode_from_bytes(&challenge_bytes).expect("decode challenge");
    let request = pdp_object([
        ("after_sequence", Value::from(0_u64)),
        ("limit", Value::from(100_u32)),
    ]);
    for (case, replacement_id) in [
        ("duplicate-id", None),
        ("duplicate-scope", Some("aa".repeat(32))),
    ] {
        let server = MockServer::start();
        let common = pdp_common_args(&server, &key_path);
        let first = pdp_pending_status(&challenge);
        let mut second = first.clone();
        let second_object = second.as_object_mut().expect("pending status object");
        second_object.insert("sequence".into(), Value::from(2_u64));
        if let Some(replacement_id) = replacement_id {
            second_object.insert("challenge_id_hex".into(), Value::from(replacement_id));
        }
        let response = pdp_object([
            ("items", Value::Array(vec![first, second])),
            ("next_sequence", Value::from(2_u64)),
        ]);
        let mock = pdp_json_mock(
            &server,
            "/v1/sorafs/pdp/export",
            &request,
            &public_key,
            "application/json",
            &response,
        );
        let output = directory.path().join(format!("{case}.json"));
        let args = vec![format!("--out={}", output.display())];
        let assert = pdp_assert("export", &common, &args).failure();
        assert!(
            String::from_utf8_lossy(&assert.get_output().stderr)
                .contains("duplicate challenge IDs or provider scopes")
        );
        assert!(!output.exists());
        mock.assert_calls(1);
    }
}
#[cfg(any(
    target_os = "android",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos"
))]
#[test]
fn pdp_export_rejects_oversized_response_before_output() {
    let directory = tempdir().expect("create PDP oversized-response tempdir");
    let server = MockServer::start();
    let (key_path, _) = pdp_operator_key(directory.path(), Algorithm::Ed25519);
    let common = pdp_common_args(&server, &key_path);
    let mock = server.mock(|when, then| {
        when.method(POST).path("/v1/sorafs/pdp/export");
        then.status(200)
            .header("Content-Type", "application/json")
            .body(vec![b' '; 4 * 1024 * 1024 + 1]);
    });
    let output = directory.path().join("oversized.json");
    let args = vec![format!("--out={}", output.display())];
    let assert = pdp_assert("export", &common, &args).failure();
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(stderr.contains("more than") || stderr.contains("exceeded"));
    assert!(!output.exists());
    mock.assert_calls(1);
}
#[cfg(unix)]
#[test]
fn pdp_inputs_reject_hardlinks_and_group_world_writable_files_before_network() {
    use std::os::unix::fs::PermissionsExt as _;
    let directory = tempdir().expect("create PDP input-policy tempdir");
    let server = MockServer::start();
    let (key_path, _) = pdp_operator_key(directory.path(), Algorithm::Ed25519);
    let common = pdp_common_args(&server, &key_path);
    let challenge_path = pdp_fixture("challenge_v1.to");
    let challenge_bytes = fs::read(&challenge_path).expect("read challenge");
    let challenge: PdpChallengeV1 = decode_from_bytes(&challenge_bytes).expect("decode challenge");
    let route = server.mock(|when, then| {
        when.method(POST).path("/v1/sorafs/pdp/challenge");
        then.status(500);
    });
    let copied = directory.path().join("commitment-copy.to");
    let linked = directory.path().join("commitment-link.to");
    fs::copy(pdp_fixture("commitment_v1.to"), &copied).expect("copy commitment");
    fs::hard_link(&copied, &linked).expect("hard-link commitment");
    let args = vec![
        format!("--commitment={}", copied.display()),
        format!("--challenge={}", challenge_path.display()),
        format!("--expected-epoch-id={}", challenge.epoch_id),
    ];
    let assert = pdp_assert("enqueue", &common, &args).failure();
    assert!(String::from_utf8_lossy(&assert.get_output().stderr).contains("hard link"));
    fs::remove_file(&linked).expect("remove extra hard link");
    fs::set_permissions(&copied, fs::Permissions::from_mode(0o666))
        .expect("make commitment writable");
    let assert = pdp_assert("enqueue", &common, &args).failure();
    assert!(String::from_utf8_lossy(&assert.get_output().stderr).contains("group or world write"));
    route.assert_calls(0);
}
#[cfg(any(
    target_os = "android",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos"
))]
#[test]
fn pdp_output_rejects_non_private_parent_before_network() {
    use std::os::unix::fs::PermissionsExt as _;
    let directory = tempdir().expect("create PDP output-parent tempdir");
    let server = MockServer::start();
    let (key_path, _) = pdp_operator_key(directory.path(), Algorithm::Ed25519);
    let common = pdp_common_args(&server, &key_path);
    let challenge_bytes = fs::read(pdp_fixture("challenge_v1.to")).expect("read challenge");
    let challenge: PdpChallengeV1 = decode_from_bytes(&challenge_bytes).expect("decode challenge");
    let writable_parent = directory.path().join("unsafe-parent");
    fs::create_dir(&writable_parent).expect("create unsafe output parent");
    let route = server.mock(|when, then| {
        when.method(POST).path("/v1/sorafs/pdp/next");
        then.status(500);
    });
    let output = writable_parent.join("challenge.to");
    let args = vec![
        format!("--provider-id-hex={}", hex_encode(challenge.provider_id)),
        format!("--challenge-out={}", output.display()),
    ];
    for mode in [0o755, 0o1777] {
        fs::set_permissions(&writable_parent, fs::Permissions::from_mode(mode))
            .expect("set non-private output parent permissions");
        let assert = pdp_assert("next", &common, &args).failure();
        assert!(
            String::from_utf8_lossy(&assert.get_output().stderr)
                .contains("current-EUID-owned mode 0700")
        );
        assert!(!output.exists());
    }
    route.assert_calls(0);
    fs::set_permissions(&writable_parent, fs::Permissions::from_mode(0o700))
        .expect("restore output parent permissions");
}
#[cfg(not(any(
    target_os = "android",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos"
)))]
#[test]
fn pdp_file_outputs_fail_closed_without_private_descriptor_relative_creation() {
    let directory = tempdir().expect("create unsupported-output tempdir");
    let server = MockServer::start();
    let (key_path, _) = pdp_operator_key(directory.path(), Algorithm::Ed25519);
    let common = pdp_common_args(&server, &key_path);
    let challenge_bytes = fs::read(pdp_fixture("challenge_v1.to")).expect("read challenge");
    let challenge: PdpChallengeV1 = decode_from_bytes(&challenge_bytes).expect("decode challenge");
    let route = server.mock(|when, then| {
        when.method(POST);
        then.status(500);
    });
    let next_output = directory.path().join("challenge.to");
    let next_args = vec![
        format!("--provider-id-hex={}", hex_encode(challenge.provider_id)),
        format!("--challenge-out={}", next_output.display()),
    ];
    let next = pdp_assert("next", &common, &next_args).failure();
    assert!(
        String::from_utf8_lossy(&next.get_output().stderr)
            .contains("lacks private descriptor-relative creation")
    );
    let export_output = directory.path().join("export.json");
    let export_args = vec![format!("--out={}", export_output.display())];
    let export = pdp_assert("export", &common, &export_args).failure();
    assert!(
        String::from_utf8_lossy(&export.get_output().stderr)
            .contains("lacks private descriptor-relative creation")
    );
    assert!(!next_output.exists() && !export_output.exists());
    route.assert_calls(0);
}
