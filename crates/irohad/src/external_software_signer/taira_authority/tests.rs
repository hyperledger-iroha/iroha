//! Focused authority protocol, durable-state, and service tests.

mod public_soak_native_tests;

use super::super::{
    SoftwareSignerKeyAlgorithmV1, SoftwareSignerPurposeBindingV1, SoftwareSignerRoleV1,
    SoftwareSignerWrappingKeyV1,
};
use super::{
    TairaAuthorityClientV1, TairaAuthorityEndpointPolicyV1, TairaAuthorityInstallationV1,
    TairaAuthorityProvisioningV1, TairaAuthorityPublicBindingV1, TairaAuthorityRoleV1,
    TairaAuthorityServerV1, TairaAuthorityServiceV1,
    protocol::{
        AuthorityAdminCommandV1, AuthorityAdminRequestV1, AuthorityFrameV1, AuthorizeRequestV1,
        FRAME_ADMIN_REQUEST_V1, FRAME_ADMIN_RESPONSE_V1, FRAME_AUTHORIZE_REQUEST_V1,
        FRAME_QUALIFY_REQUEST_V1, FRAME_QUALIFY_RESPONSE_V1, OperationResponseV1,
        OperationStatusV1, QualifyRequestV1, QualifyResponseV1, ReplayConsumptionV1,
        StoredAuthorizationV1, StoredPublicSoakObservationBindingAnchorV1,
        StoredPublicSoakObservationBindingInputV1, TAIRA_AUTHORITY_MAX_FRAME_BYTES_V1,
        TAIRA_AUTHORITY_PROTOCOL_MAGIC_V1, TAIRA_AUTHORITY_PROTOCOL_VERSION_V1, decode_body,
        decode_frame, encode_frame, qualify_response_digest,
    },
    service::{
        AuthorityProcessIdentityModeV1, DeploymentFinalizationCrashPhaseV1,
        GenericAuthorizationCrashPhaseV1, PublicSoakBindingCrashPhaseV1,
        PublicSoakBindingProvisioningModeV1, TairaAuthorityErrorV1,
        artifact_is_authority_immutable, digest_parts_sha256,
    },
    store::{PersistOutcomeV1, load_canonical_records, persist_canonical_once},
    transport::serve_one_for_test,
    validate_taira_authority_installations_v1, validate_taira_authority_registry_v1,
};
use iroha_crypto::{Algorithm, KeyPair};
use norito::json::{Map, Value};
use sha2::{Digest as _, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::{Read as _, Write as _},
    os::{
        fd::OwnedFd,
        unix::fs::{DirBuilderExt as _, OpenOptionsExt as _, PermissionsExt as _},
        unix::net::UnixStream,
    },
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

const TEST_WRAPPING_KEY_V1: [u8; 32] = [0xA7; 32];
const TEST_NOW_MILLIS_V1: u64 = 1_900_000_000_000;
const RUN_ID_DOMAIN_V1: &[u8] = b"iroha:taira:authority-run-id:v1\0";
const OPERATION_ID_DOMAIN_V1: &[u8] = b"iroha:taira:authority-operation-id:v1\0";
const TEST_NATIVE_CONTROLLER_DIGEST_V1: [u8; 32] = [0xC1; 32];
const TEST_NATIVE_RUN_NONCE_V1: [u8; 32] = [0xA5; 32];
const TEST_NATIVE_CONTROLLER_HOST_ID_V1: &str = "native-evidence-host-test-v1";
const TEST_NATIVE_CONTROLLER_INSTALLATION_ID_V1: &str =
    "native-evidence-controller-installation-test-v1";

impl TairaAuthorityServiceV1 {
    /// Provision a role with a synthetic service UID for the isolated in-process test harness.
    pub(super) fn provision_for_test(
        state_directory: impl Into<PathBuf>,
        provisioning: TairaAuthorityProvisioningV1,
        wrapping_key: SoftwareSignerWrappingKeyV1,
    ) -> Result<Self, TairaAuthorityErrorV1> {
        Self::provision_inner(
            state_directory.into(),
            provisioning,
            wrapping_key,
            None,
            None,
            PublicSoakBindingProvisioningModeV1::Complete,
            AuthorityProcessIdentityModeV1::SyntheticTest,
        )
    }

    /// Recover synthetic-UID role state owned by the current test process.
    pub(super) fn open_for_test(
        state_directory: impl Into<PathBuf>,
        wrapping_key: SoftwareSignerWrappingKeyV1,
    ) -> Result<Self, TairaAuthorityErrorV1> {
        Self::open_inner(
            state_directory,
            wrapping_key,
            AuthorityProcessIdentityModeV1::SyntheticTest,
        )
    }
}

fn wrapping_key() -> SoftwareSignerWrappingKeyV1 {
    SoftwareSignerWrappingKeyV1::try_from_bytes(TEST_WRAPPING_KEY_V1).expect("fixture wrapping key")
}

fn temporary_parent() -> tempfile::TempDir {
    tempfile::tempdir_in(std::env::current_dir().expect("current directory"))
        .expect("secure temporary authority parent")
}

fn provisioning(role: TairaAuthorityRoleV1) -> TairaAuthorityProvisioningV1 {
    let service_uid = rustix::process::geteuid().as_raw();
    TairaAuthorityProvisioningV1 {
        role,
        service_id: format!("taira-authority-{}-fixture-v1", role.as_str()),
        administrator_id: format!("taira-authority-{}-administrator-fixture-v1", role.as_str()),
        service_uid,
        client_uid: service_uid.checked_add(1).expect("fixture client UID"),
        administrator_uid: service_uid
            .checked_add(2)
            .expect("fixture administrator UID"),
        key_revision: 1,
        policy_revision: 1,
        policy_digest: [0x51; 32],
        max_request_bytes: 1024 * 1024,
    }
}

fn provision(parent: &Path, role: TairaAuthorityRoleV1) -> TairaAuthorityServiceV1 {
    TairaAuthorityServiceV1::provision(
        parent.join("authority-state"),
        provisioning(role),
        wrapping_key(),
    )
    .expect("provision fixture authority")
}

fn canonical_json_core(value: &Value) -> Vec<u8> {
    norito::json::to_json(value)
        .expect("serialize canonical fixture JSON")
        .into_bytes()
}

fn canonical_json_line(value: &Value) -> Vec<u8> {
    let mut bytes = canonical_json_core(value);
    bytes.push(b'\n');
    bytes
}

fn parse_json(bytes: &[u8]) -> Value {
    norito::json::from_slice(bytes).expect("parse fixture JSON")
}

#[derive(Clone, Debug)]
struct ClientRequestFixtureV1 {
    role: TairaAuthorityRoleV1,
    subject: Value,
    manifest: Value,
    run_id: [u8; 32],
    operation_id: [u8; 32],
    subject_sha256: [u8; 32],
    manifest_sha256: [u8; 32],
}

impl ClientRequestFixtureV1 {
    fn new(role: TairaAuthorityRoleV1, case: &str, artifacts: &[(&str, &[u8])]) -> Self {
        let subject = if role == TairaAuthorityRoleV1::RolloutObservation {
            super::rollout_observation::tests::valid_subject_for_case(case)
        } else {
            let mut subject = Map::new();
            subject.insert("case".into(), Value::from(case));
            subject.insert("schema_version".into(), Value::from(1_u64));
            Value::Object(subject)
        };
        let manifest = Value::Array(
            artifacts
                .iter()
                .enumerate()
                .map(|(ordinal, (name, bytes))| {
                    let mut row = Map::new();
                    row.insert("name".into(), Value::from(*name));
                    row.insert("ordinal".into(), Value::from(ordinal as u64));
                    row.insert(
                        "sha256".into(),
                        Value::from(hex::encode(Sha256::digest(bytes))),
                    );
                    row.insert("size".into(), Value::from(bytes.len() as u64));
                    Value::Object(row)
                })
                .collect(),
        );
        Self::from_subject_and_manifest(role, subject, manifest)
    }

    fn from_subject_and_manifest(
        role: TairaAuthorityRoleV1,
        subject: Value,
        manifest: Value,
    ) -> Self {
        let subject_sha256: [u8; 32] = Sha256::digest(canonical_json_core(&subject)).into();
        let manifest_sha256: [u8; 32] = Sha256::digest(canonical_json_core(&manifest)).into();
        let run_id = digest_parts_sha256(
            RUN_ID_DOMAIN_V1,
            &[role.as_str().as_bytes(), &subject_sha256],
        );
        let operation_id = digest_parts_sha256(
            OPERATION_ID_DOMAIN_V1,
            &[
                role.as_str().as_bytes(),
                &run_id,
                &subject_sha256,
                &manifest_sha256,
            ],
        );
        Self {
            role,
            subject,
            manifest,
            run_id,
            operation_id,
            subject_sha256,
            manifest_sha256,
        }
    }

    fn request_json(&self) -> Vec<u8> {
        self.request_json_with_deploy(None, None)
    }

    fn request_json_with_deploy(
        &self,
        disposition: Option<&str>,
        deployment_result: Option<(&str, [u8; 32])>,
    ) -> Vec<u8> {
        let mut request = Map::new();
        request.insert("artifact_manifest".into(), self.manifest.clone());
        request.insert(
            "operation_id".into(),
            Value::from(hex::encode(self.operation_id)),
        );
        request.insert("role".into(), Value::from(self.role.as_str()));
        request.insert("run_id".into(), Value::from(hex::encode(self.run_id)));
        request.insert(
            "schema".into(),
            Value::from("iroha.taira.authority-client-request.v1"),
        );
        request.insert("subject".into(), self.subject.clone());
        if let Some(disposition) = disposition {
            request.insert("disposition".into(), Value::from(disposition));
        }
        if let Some((outcome, result_sha256)) = deployment_result {
            let mut result = Map::new();
            result.insert("outcome".into(), Value::from(outcome));
            result.insert(
                "result_sha256".into(),
                Value::from(hex::encode(result_sha256)),
            );
            request.insert("deployment_result".into(), Value::Object(result));
        }
        canonical_json_line(&Value::Object(request))
    }

    fn assignment_json(
        &self,
        service: &TairaAuthorityServiceV1,
        issued_at: u64,
        not_before: u64,
        expires_at: u64,
    ) -> Vec<u8> {
        let binding = service.public_binding().expect("fixture binding");
        let mut assignment = Map::new();
        assignment.insert(
            "artifact_manifest_sha256".into(),
            Value::from(hex::encode(self.manifest_sha256)),
        );
        if self.role == TairaAuthorityRoleV1::NativeEvidence {
            assignment.insert(
                "controller_host_id".into(),
                Value::from(TEST_NATIVE_CONTROLLER_HOST_ID_V1),
            );
            assignment.insert(
                "controller_installation_id".into(),
                Value::from(TEST_NATIVE_CONTROLLER_INSTALLATION_ID_V1),
            );
            assignment.insert(
                "controller_digest".into(),
                Value::from(hex::encode(TEST_NATIVE_CONTROLLER_DIGEST_V1)),
            );
            assignment.insert(
                "run_nonce".into(),
                Value::from(hex::encode(TEST_NATIVE_RUN_NONCE_V1)),
            );
        }
        assignment.insert("expires_at_unix_millis".into(), Value::from(expires_at));
        assignment.insert("issued_at_unix_millis".into(), Value::from(issued_at));
        assignment.insert(
            "key_revision".into(),
            Value::from(binding.signer.key_revision),
        );
        assignment.insert("not_before_unix_millis".into(), Value::from(not_before));
        assignment.insert(
            "policy_revision".into(),
            Value::from(binding.signer.policy_revision),
        );
        assignment.insert(
            "policy_sha256".into(),
            Value::from(hex::encode(binding.signer.policy_digest)),
        );
        assignment.insert("role".into(), Value::from(self.role.as_str()));
        assignment.insert("run_id".into(), Value::from(hex::encode(self.run_id)));
        assignment.insert(
            "schema".into(),
            Value::from("iroha.taira.authority-run-assignment.v1"),
        );
        assignment.insert(
            "subject_sha256".into(),
            Value::from(hex::encode(self.subject_sha256)),
        );
        canonical_json_line(&Value::Object(assignment))
    }
}

fn assign_active_run(service: &TairaAuthorityServiceV1, fixture: &ClientRequestFixtureV1) {
    let response = service
        .assign_run_json(
            &fixture.assignment_json(
                service,
                TEST_NOW_MILLIS_V1 - 10,
                TEST_NOW_MILLIS_V1 - 1,
                TEST_NOW_MILLIS_V1 + 60_000,
            ),
            TEST_NOW_MILLIS_V1,
        )
        .expect("assign fixture run");
    assert_eq!(response.status, OperationStatusV1::Ok);
}

fn create_artifact(parent: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let path = parent.join(name);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .expect("create fixture artifact");
    file.write_all(bytes).expect("write fixture artifact");
    file.sync_all().expect("sync fixture artifact");
    drop(file);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o400))
        .expect("make fixture artifact immutable");
    path
}

fn read_only_descriptors(paths: &[&Path]) -> Vec<OwnedFd> {
    paths
        .iter()
        .map(|path| {
            let file = OpenOptions::new()
                .read(true)
                .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits().cast_signed())
                .open(path)
                .expect("open fixture descriptor");
            OwnedFd::from(file)
        })
        .collect()
}

fn authorize(
    service: &TairaAuthorityServiceV1,
    request_json: &[u8],
    descriptors: Vec<OwnedFd>,
    now: u64,
) -> Result<OperationResponseV1, TairaAuthorityErrorV1> {
    service.authorize_json(
        request_json,
        descriptors,
        service.public_binding()?.signer.client_uid,
        now,
    )
}

fn sidecar_bytes(response: &OperationResponseV1, field: &str) -> Vec<u8> {
    let result = parse_json(&response.result_json);
    canonical_json_line(result.get(field).expect("result sidecar"))
}

fn verification_json(
    fixture: &ClientRequestFixtureV1,
    authorized: &OperationResponseV1,
) -> Vec<u8> {
    let authorized = parse_json(&authorized.result_json);
    let mut request = Map::new();
    request.insert("artifact_manifest".into(), fixture.manifest.clone());
    request.insert(
        "authority_envelope".into(),
        authorized
            .get("authority_envelope")
            .cloned()
            .expect("authority envelope"),
    );
    request.insert(
        "durable_receipt".into(),
        authorized
            .get("durable_receipt")
            .cloned()
            .expect("durable receipt"),
    );
    request.insert(
        "operation_id".into(),
        Value::from(hex::encode(fixture.operation_id)),
    );
    request.insert("role".into(), Value::from(fixture.role.as_str()));
    request.insert("run_id".into(), Value::from(hex::encode(fixture.run_id)));
    request.insert(
        "schema".into(),
        Value::from("iroha.taira.authority-client-verification.v1"),
    );
    request.insert("subject".into(), fixture.subject.clone());
    canonical_json_line(&Value::Object(request))
}

fn result_status(response: &OperationResponseV1) -> String {
    let value = parse_json(&response.result_json);
    value
        .get("status")
        .and_then(Value::as_str)
        .expect("result status")
        .to_owned()
}

fn state_directory(parent: &Path) -> PathBuf {
    parent.join("authority-state")
}

fn public_soak_anchor_state_directory(parent: &Path) -> PathBuf {
    parent.join("public-soak-replay-anchor-state")
}

fn independent_public_soak_observation_binding(parent: &Path) -> TairaAuthorityPublicBindingV1 {
    let observation = TairaAuthorityServiceV1::provision(
        parent.join("public-soak-observation-anchor-state"),
        provisioning(TairaAuthorityRoleV1::PublicSoakObservation),
        wrapping_key(),
    )
    .expect("provision observation anchor fixture");
    let mut binding = observation
        .public_binding()
        .expect("observation anchor binding");
    let effective_uid = rustix::process::geteuid().as_raw();
    binding.signer.service_uid = effective_uid
        .checked_add(10)
        .expect("observation service UID fixture");
    binding.signer.client_uid = effective_uid
        .checked_add(11)
        .expect("observation client UID fixture");
    binding.signer.administrator_uid = effective_uid
        .checked_add(12)
        .expect("observation administrator UID fixture");
    binding
        .validate()
        .expect("synthetic independently owned observation binding");
    binding
}

fn provision_public_soak_anchor_fixture(
    parent: &Path,
) -> (
    TairaAuthorityServiceV1,
    StoredPublicSoakObservationBindingInputV1,
    StoredPublicSoakObservationBindingAnchorV1,
) {
    let observation_binding = independent_public_soak_observation_binding(parent);
    let state = public_soak_anchor_state_directory(parent);
    let service = TairaAuthorityServiceV1::provision_with_public_soak_observation_binding(
        &state,
        provisioning(TairaAuthorityRoleV1::PublicSoakReplayAdmission),
        wrapping_key(),
        observation_binding,
    )
    .expect("provision replay anchor fixture");
    let inputs: BTreeMap<[u8; 32], StoredPublicSoakObservationBindingInputV1> =
        load_canonical_records(&state.join("public-soak-observation-binding-input-v1"))
            .expect("load observation binding write-ahead input");
    let anchors: BTreeMap<[u8; 32], StoredPublicSoakObservationBindingAnchorV1> =
        load_canonical_records(&state.join("public-soak-observation-binding-v1"))
            .expect("load signed observation binding anchor");
    let [input] = inputs
        .into_values()
        .collect::<Vec<_>>()
        .try_into()
        .expect("one observation binding write-ahead input");
    let [anchor] = anchors
        .into_values()
        .collect::<Vec<_>>()
        .try_into()
        .expect("one signed observation binding anchor");
    (service, input, anchor)
}

fn record_path(directory: &Path, key: [u8; 32]) -> PathBuf {
    directory.join(format!("{}.norito", hex::encode(key)))
}

fn pending_record_path(directory: &Path, key: [u8; 32]) -> PathBuf {
    directory.join(format!(".{}.pending", hex::encode(key)))
}

fn audit_record_path(state: &Path, sequence: u64) -> PathBuf {
    state
        .join("audit-v1")
        .join(format!("{sequence:020}.norito"))
}

fn pending_audit_record_path(state: &Path, sequence: u64) -> PathBuf {
    state
        .join("audit-v1")
        .join(format!(".pending-{sequence:020}.norito"))
}

fn registry_provisioning(
    role: TairaAuthorityRoleV1,
    ordinal: usize,
) -> TairaAuthorityProvisioningV1 {
    let base_uid = 60_000_u32
        .checked_add(u32::try_from(ordinal).expect("role ordinal") * 3)
        .expect("fixture UID range");
    TairaAuthorityProvisioningV1 {
        role,
        service_id: format!("taira-authority-{}-registry-fixture-v1", role.as_str()),
        administrator_id: format!(
            "taira-authority-{}-registry-administrator-fixture-v1",
            role.as_str()
        ),
        service_uid: if role == TairaAuthorityRoleV1::Qualification {
            0
        } else {
            base_uid
        },
        client_uid: base_uid + 1,
        administrator_uid: base_uid + 2,
        key_revision: 1,
        policy_revision: 1,
        policy_digest: [u8::try_from(ordinal + 1).expect("policy fixture"); 32],
        max_request_bytes: 1024 * 1024,
    }
}

fn registry_wrapping_key(ordinal: usize) -> SoftwareSignerWrappingKeyV1 {
    SoftwareSignerWrappingKeyV1::try_from_bytes(
        [0xB0_u8
            .checked_add(u8::try_from(ordinal).expect("role ordinal"))
            .expect("fixture wrapping key range"); 32],
    )
    .expect("registry wrapping key")
}

fn provision_eight_role_harness(
    parent: &Path,
) -> (
    Vec<Arc<TairaAuthorityServiceV1>>,
    Vec<TairaAuthorityInstallationV1>,
) {
    let mut services = Vec::with_capacity(TairaAuthorityRoleV1::ALL.len());
    let mut installations = Vec::with_capacity(TairaAuthorityRoleV1::ALL.len());
    for (ordinal, role) in TairaAuthorityRoleV1::ALL.into_iter().enumerate() {
        let state = parent.join(format!("{:02}-{}-state", ordinal, role.as_str()));
        let provisioning = registry_provisioning(role, ordinal);
        let service = match role {
            TairaAuthorityRoleV1::PrivacyGovernance => {
                let retained = KeyPair::try_random_with_algorithm(Algorithm::Ed25519)
                    .expect("retained genesis fixture key");
                TairaAuthorityServiceV1::provision_with_retained_genesis_key_for_test(
                    &state,
                    provisioning,
                    registry_wrapping_key(ordinal),
                    retained,
                )
            }
            TairaAuthorityRoleV1::PublicSoakReplayAdmission => {
                let observation = services
                    .iter()
                    .find_map(|service: &Arc<TairaAuthorityServiceV1>| {
                        let binding = service.public_binding().ok()?;
                        (binding.role == TairaAuthorityRoleV1::PublicSoakObservation)
                            .then_some(binding)
                    })
                    .expect("independent public-soak observation binding");
                TairaAuthorityServiceV1::provision_with_public_soak_observation_binding_for_test(
                    &state,
                    provisioning,
                    registry_wrapping_key(ordinal),
                    observation,
                )
            }
            _ => TairaAuthorityServiceV1::provision_for_test(
                &state,
                provisioning,
                registry_wrapping_key(ordinal),
            ),
        }
        .expect("provision isolated harness role");
        let binding = service.public_binding().expect("harness binding");
        installations.push(TairaAuthorityInstallationV1 {
            binding,
            state_directory: state,
            request_socket: parent.join(format!("{:02}-{}-request.sock", ordinal, role.as_str())),
            administrator_socket: parent.join(format!(
                "{:02}-{}-administrator.sock",
                ordinal,
                role.as_str()
            )),
        });
        services.push(Arc::new(service));
    }
    (services, installations)
}

fn direct_transport_round_trip(
    service: Arc<TairaAuthorityServiceV1>,
    administrator: bool,
    authenticated_uid: u32,
    encoded_request: &[u8],
) -> AuthorityFrameV1 {
    let (mut client, server) = UnixStream::pair().expect("local authority stream pair");
    client
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("set direct transport timeout");
    let worker = std::thread::spawn(move || {
        serve_one_for_test(server, administrator, authenticated_uid, &service)
    });
    let length = u32::try_from(encoded_request.len()).expect("request frame length");
    client
        .write_all(&length.to_be_bytes())
        .and_then(|()| client.write_all(encoded_request))
        .and_then(|()| client.flush())
        .expect("send direct authority frame");
    let mut prefix = [0_u8; 4];
    client
        .read_exact(&mut prefix)
        .expect("read authority response prefix");
    let response_length = usize::try_from(u32::from_be_bytes(prefix)).expect("response length");
    assert!(response_length <= TAIRA_AUTHORITY_MAX_FRAME_BYTES_V1);
    let mut response = vec![0_u8; response_length];
    client
        .read_exact(&mut response)
        .expect("read authority response frame");
    worker
        .join()
        .expect("join authority transport worker")
        .expect("serve direct authority request");
    decode_frame(&response).expect("decode authority response frame")
}

#[test]
fn role_registry_is_exact_and_stable() {
    assert_eq!(TairaAuthorityRoleV1::ALL.len(), 8);
    assert_eq!(
        TairaAuthorityRoleV1::ALL.map(TairaAuthorityRoleV1::as_str),
        [
            "native-evidence",
            "privacy-protocol-origin",
            "privacy-governance",
            "qualification",
            "deploy-issuance",
            "rollout-observation",
            "public-soak-observation",
            "public-soak-replay-admission",
        ]
    );
    for role in TairaAuthorityRoleV1::ALL {
        assert_eq!(role.as_str().parse(), Ok(role));
    }
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "end-to-end eight-role isolation regression"
)]
fn local_eight_role_service_transport_harness_is_fully_isolated() {
    let parent = temporary_parent();
    let (services, installations) = provision_eight_role_harness(parent.path());
    let bindings = installations
        .iter()
        .map(|installation| installation.binding.clone())
        .collect::<Vec<_>>();
    validate_taira_authority_registry_v1(&bindings).expect("complete isolated role registry");
    validate_taira_authority_installations_v1(&installations)
        .expect("complete isolated installation registry");

    let mut identities = BTreeSet::new();
    let mut uids = BTreeSet::new();
    let mut public_keys = BTreeSet::new();
    let mut encrypted_keys = BTreeSet::new();
    let mut state_directories = BTreeSet::new();
    let mut sockets = BTreeSet::new();
    for (ordinal, (service, installation)) in services.iter().zip(&installations).enumerate() {
        let binding = &installation.binding;
        assert_eq!(binding.role, TairaAuthorityRoleV1::ALL[ordinal]);
        assert!(identities.insert(binding.signer.service_id.clone()));
        assert!(identities.insert(binding.signer.administrator_id.clone()));
        assert!(uids.insert(binding.signer.service_uid));
        assert!(uids.insert(binding.signer.client_uid));
        assert!(uids.insert(binding.signer.administrator_uid));
        assert!(public_keys.insert(binding.signer.public_key_digest));
        assert!(state_directories.insert(installation.state_directory.clone()));
        assert!(sockets.insert(installation.request_socket.clone()));
        assert!(sockets.insert(installation.administrator_socket.clone()));
        let encrypted_key = fs::read(installation.state_directory.join("key-envelope-v1.norito"))
            .expect("read encrypted role key");
        assert!(!encrypted_key.is_empty());
        assert!(encrypted_keys.insert(Sha256::digest(encrypted_key).to_vec()));

        let policy = TairaAuthorityEndpointPolicyV1::try_new(
            &installation.request_socket,
            &installation.administrator_socket,
            binding.clone(),
        )
        .expect("isolated endpoint policy");
        assert_eq!(policy.binding, *binding);

        let nonce = [u8::try_from(ordinal + 1).expect("nonce fixture"); 32];
        let qualify_frame = direct_transport_round_trip(
            Arc::clone(service),
            false,
            binding.signer.client_uid,
            &encode_frame(
                FRAME_QUALIFY_REQUEST_V1,
                &QualifyRequestV1 {
                    binding_sha256: binding.sha256().expect("binding digest"),
                    client_nonce: nonce,
                },
            )
            .expect("encode qualify request"),
        );
        assert_eq!(qualify_frame.kind, FRAME_QUALIFY_RESPONSE_V1);
        let qualified: QualifyResponseV1 =
            decode_body(&qualify_frame.body).expect("decode qualify response");
        assert_eq!(qualified.client_nonce, nonce);
        assert_ne!(qualified.server_nonce, [0; 32]);
        assert_eq!(qualified.provenance.binding, binding.signer);
        assert_eq!(
            qualified.response_digest,
            qualify_response_digest(&qualified).expect("qualify response digest")
        );
        assert!(!qualified.response_attestation.is_empty());
        let qualified_status = parse_json(&qualified.status_json);
        assert_eq!(qualified_status["role"], Value::from(binding.role.as_str()));
        assert_eq!(qualified_status["status"], Value::from("ready"));

        let admin_frame = direct_transport_round_trip(
            Arc::clone(service),
            true,
            binding.signer.administrator_uid,
            &encode_frame(
                FRAME_ADMIN_REQUEST_V1,
                &AuthorityAdminRequestV1 {
                    binding_sha256: binding.sha256().expect("binding digest"),
                    command: AuthorityAdminCommandV1::Status,
                },
            )
            .expect("encode administrator status request"),
        );
        assert_eq!(admin_frame.kind, FRAME_ADMIN_RESPONSE_V1);
        let administered: OperationResponseV1 =
            decode_body(&admin_frame.body).expect("decode administrator response");
        assert_eq!(administered.status, OperationStatusV1::Ok);
        assert_eq!(result_status(&administered), "ready");
    }
    assert_eq!(identities.len(), 16);
    assert_eq!(uids.len(), 24);
    assert_eq!(public_keys.len(), 8);
    assert_eq!(encrypted_keys.len(), 8);
    assert_eq!(state_directories.len(), 8);
    assert_eq!(sockets.len(), 16);

    let observation_index = installations
        .iter()
        .position(|installation| {
            installation.binding.role == TairaAuthorityRoleV1::PublicSoakObservation
        })
        .expect("observation installation");
    let replay_index = installations
        .iter()
        .position(|installation| {
            installation.binding.role == TairaAuthorityRoleV1::PublicSoakReplayAdmission
        })
        .expect("replay installation");
    let observation = &installations[observation_index];
    let replay = &installations[replay_index];
    assert_ne!(
        observation.binding.signer.handle,
        replay.binding.signer.handle
    );
    assert_ne!(
        observation.binding.signer.public_key_digest,
        replay.binding.signer.public_key_digest
    );
    assert_ne!(observation.state_directory, replay.state_directory);
    assert_ne!(observation.request_socket, replay.request_socket);
    assert_ne!(
        observation.administrator_socket,
        replay.administrator_socket
    );

    let mut duplicate_role = bindings.clone();
    duplicate_role[replay_index] = duplicate_role[observation_index].clone();
    assert!(validate_taira_authority_registry_v1(&duplicate_role).is_err());
    let mut shared_socket = installations.clone();
    shared_socket[replay_index].request_socket =
        shared_socket[observation_index].request_socket.clone();
    assert!(validate_taira_authority_installations_v1(&shared_socket).is_err());
    assert!(
        TairaAuthorityEndpointPolicyV1::try_new(
            &observation.request_socket,
            &observation.request_socket,
            observation.binding.clone(),
        )
        .is_err()
    );

    let substituted_policy = TairaAuthorityEndpointPolicyV1::try_new(
        &installations[0].request_socket,
        &installations[0].administrator_socket,
        installations[1].binding.clone(),
    )
    .expect("syntactically valid substituted endpoint policy");
    assert!(matches!(
        TairaAuthorityServerV1::try_new(Arc::clone(&services[0]), substituted_policy),
        Err(TairaAuthorityErrorV1::Binding)
    ));

    let effective_uid = rustix::process::geteuid().as_raw();
    let mismatched_service = installations
        .iter()
        .position(|installation| installation.binding.signer.service_uid != effective_uid)
        .expect("an eight-role registry always contains another service UID");
    let process_mismatch_policy = TairaAuthorityEndpointPolicyV1::try_new(
        &installations[mismatched_service].request_socket,
        &installations[mismatched_service].administrator_socket,
        installations[mismatched_service].binding.clone(),
    )
    .expect("process-mismatch policy");
    assert!(matches!(
        TairaAuthorityServerV1::try_new(
            Arc::clone(&services[mismatched_service]),
            process_mismatch_policy,
        ),
        Err(TairaAuthorityErrorV1::Binding)
    ));

    let unavailable_policy = TairaAuthorityEndpointPolicyV1::try_new(
        &installations[0].request_socket,
        &installations[0].administrator_socket,
        installations[0].binding.clone(),
    )
    .expect("unavailable endpoint policy");
    assert_eq!(
        TairaAuthorityClientV1::new(unavailable_policy).qualify(),
        Err(TairaAuthorityErrorV1::State),
        "a reviewed binding cannot stand in for service availability"
    );

    let binding = installations[0].binding.clone();
    let (mut client, server) = UnixStream::pair().expect("invalid-binding stream pair");
    let service = Arc::clone(&services[0]);
    let worker = std::thread::spawn(move || {
        serve_one_for_test(server, false, binding.signer.client_uid, &service)
    });
    let invalid = encode_frame(
        FRAME_QUALIFY_REQUEST_V1,
        &QualifyRequestV1 {
            binding_sha256: [0xFF; 32],
            client_nonce: [0x91; 32],
        },
    )
    .expect("encode invalid-binding request");
    client
        .write_all(
            &u32::try_from(invalid.len())
                .expect("frame length")
                .to_be_bytes(),
        )
        .and_then(|()| client.write_all(&invalid))
        .and_then(|()| client.flush())
        .expect("send invalid-binding request");
    assert_eq!(
        worker.join().expect("join invalid-binding worker"),
        Err(TairaAuthorityErrorV1::Binding)
    );
}

#[test]
fn identifiers_match_python_length_framed_golden_vector() {
    let subject_sha256: [u8; 32] = Sha256::digest(br#"{"a":1,"b":2}"#).into();
    let manifest_sha256: [u8; 32] = Sha256::digest(
        br#"[{"name":"evidence/one","ordinal":0,"sha256":"7692c3ad3540bb803c020b3aee66cd8887123234ea0c6e7143c0add73ff431ed","size":3}]"#,
    )
    .into();
    let run_id = digest_parts_sha256(
        b"iroha:taira:authority-run-id:v1\0",
        &[b"native-evidence", &subject_sha256],
    );
    assert_eq!(
        hex::encode(run_id),
        "e0f3893153fa637143efe8ba0119af50776b822bab7300d638b424e74096b357"
    );
    assert_eq!(
        hex::encode(digest_parts_sha256(
            b"iroha:taira:authority-operation-id:v1\0",
            &[
                b"native-evidence",
                &run_id,
                &subject_sha256,
                &manifest_sha256,
            ],
        )),
        "b7bb54043e91ce00c7333bf880c2c0b2dd2fab5d2edf4ca48525a30917c2f647"
    );
}

#[test]
fn provision_assign_authorize_retry_verify_and_recover() {
    let parent = temporary_parent();
    let artifact = create_artifact(parent.path(), "observation.json", b"observation-v1");
    let fixture = ClientRequestFixtureV1::new(
        TairaAuthorityRoleV1::RolloutObservation,
        "authorize-recovery",
        &[("observation.json", b"observation-v1")],
    );
    let service = provision(parent.path(), fixture.role);
    assert_eq!(
        parse_json(&service.status_json().expect("initial status"))["status"],
        Value::from("ready")
    );
    assign_active_run(&service, &fixture);

    let fresh = authorize(
        &service,
        &fixture.request_json(),
        read_only_descriptors(&[&artifact]),
        TEST_NOW_MILLIS_V1,
    )
    .expect("fresh authorization");
    assert_eq!(fresh.status, OperationStatusV1::Ok);
    assert_eq!(result_status(&fresh), "authorized");
    let envelope = sidecar_bytes(&fresh, "authority_envelope");
    let durable_receipt = sidecar_bytes(&fresh, "durable_receipt");

    let audit_after_fresh = service.provenance().expect("fresh provenance");
    let replay = authorize(
        &service,
        &fixture.request_json(),
        read_only_descriptors(&[&artifact]),
        TEST_NOW_MILLIS_V1 + 1,
    )
    .expect("exact retry");
    assert_eq!(replay.status, OperationStatusV1::Replayed);
    assert_eq!(result_status(&replay), "replayed");
    assert_eq!(sidecar_bytes(&replay, "authority_envelope"), envelope);
    assert_eq!(sidecar_bytes(&replay, "durable_receipt"), durable_receipt);
    assert_eq!(
        service.provenance().expect("replay provenance").audit_head,
        audit_after_fresh.audit_head,
        "an exact retry must not append another audit record"
    );

    let verified = service
        .verify_json(
            &verification_json(&fixture, &fresh),
            read_only_descriptors(&[&artifact]),
            service
                .public_binding()
                .expect("verification binding")
                .signer
                .client_uid,
        )
        .expect("historical verification");
    assert_eq!(verified.status, OperationStatusV1::Ok);
    assert_eq!(result_status(&verified), "valid");
    assert_eq!(
        service.provenance().expect("verify provenance").audit_head,
        audit_after_fresh.audit_head,
        "historical verification must not sign"
    );

    drop(service);
    let state = state_directory(parent.path());
    let receipts = state.join("authority-receipts-v1");
    fs::rename(
        record_path(&receipts, fixture.operation_id),
        pending_record_path(&receipts, fixture.operation_id),
    )
    .expect("simulate receipt promotion crash");
    let recovered =
        TairaAuthorityServiceV1::open(&state, wrapping_key()).expect("open recovered authority");
    assert!(record_path(&receipts, fixture.operation_id).is_file());
    assert!(!pending_record_path(&receipts, fixture.operation_id).exists());
    let replay_after_recovery = authorize(
        &recovered,
        &fixture.request_json(),
        read_only_descriptors(&[&artifact]),
        TEST_NOW_MILLIS_V1 + 2,
    )
    .expect("retry recovered authorization");
    assert_eq!(replay_after_recovery.status, OperationStatusV1::Replayed);
    assert_eq!(
        sidecar_bytes(&replay_after_recovery, "authority_envelope"),
        envelope
    );
    assert_eq!(
        sidecar_bytes(&replay_after_recovery, "durable_receipt"),
        durable_receipt
    );
    assert_eq!(
        recovered.provenance().expect("recovered provenance"),
        audit_after_fresh
    );
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "durable crash-boundary regression matrix"
)]
fn generic_authorization_recovers_each_durable_crash_boundary_after_run_expiry() {
    for (case, phase) in [
        (
            "crash-after-consumption-persistence",
            GenericAuthorizationCrashPhaseV1::AfterConsumptionPersistence,
        ),
        (
            "crash-after-envelope-signer-commit",
            GenericAuthorizationCrashPhaseV1::AfterEnvelopeSignerCommit,
        ),
        (
            "crash-after-durable-receipt-signer-commit",
            GenericAuthorizationCrashPhaseV1::AfterDurableReceiptSignerCommit,
        ),
    ] {
        let parent = temporary_parent();
        let fixture =
            ClientRequestFixtureV1::new(TairaAuthorityRoleV1::RolloutObservation, case, &[]);
        let service = provision(parent.path(), fixture.role);
        assign_active_run(&service, &fixture);
        service
            .inject_generic_authorization_crash_for_test(phase)
            .expect("configure isolated crash phase");
        assert_eq!(
            authorize(
                &service,
                &fixture.request_json(),
                Vec::new(),
                TEST_NOW_MILLIS_V1,
            ),
            Err(TairaAuthorityErrorV1::State),
            "generic authorization did not stop at {phase:?}"
        );
        let audit_at_crash = service.provenance().expect("crash-boundary provenance");
        assert_eq!(
            parse_json(
                &service
                    .status_json()
                    .expect("status during incomplete authorization")
            )["status"],
            Value::from("ready"),
            "status must remain available during {phase:?}"
        );
        let blocked_assignment = ClientRequestFixtureV1::new(
            TairaAuthorityRoleV1::RolloutObservation,
            &format!("{case}-blocked-assignment"),
            &[],
        );
        assert_eq!(
            service.assign_run_json(
                &blocked_assignment.assignment_json(
                    &service,
                    TEST_NOW_MILLIS_V1 - 10,
                    TEST_NOW_MILLIS_V1 - 1,
                    TEST_NOW_MILLIS_V1 + 60_000,
                ),
                TEST_NOW_MILLIS_V1,
            ),
            Err(TairaAuthorityErrorV1::Conflict),
            "assign-run appended across incomplete authorization at {phase:?}"
        );
        assert_eq!(
            service.administer(
                AuthorityAdminCommandV1::Revoke {
                    operation_id: digest_parts_sha256(
                        b"iroha:taira:test-incomplete-revoke:v1\0",
                        &[case.as_bytes()],
                    ),
                    expected_audit_head: audit_at_crash.audit_head,
                    expected_key_revision: service
                        .public_binding()
                        .expect("incomplete revoke binding")
                        .signer
                        .key_revision,
                    reason_digest: [0xE7; 32],
                },
                TEST_NOW_MILLIS_V1,
            ),
            Err(TairaAuthorityErrorV1::Conflict),
            "revoke appended across incomplete authorization at {phase:?}"
        );
        assert_eq!(
            service.provenance().expect("blocked admin provenance"),
            audit_at_crash,
            "blocked admin command changed the audit head at {phase:?}"
        );
        drop(service);

        let recovered =
            TairaAuthorityServiceV1::open(state_directory(parent.path()), wrapping_key())
                .expect("recover authority after injected crash");
        let completed = authorize(
            &recovered,
            &fixture.request_json(),
            Vec::new(),
            TEST_NOW_MILLIS_V1 + 60_001,
        )
        .expect("complete persisted admission after original assignment expiry");
        assert_eq!(completed.status, OperationStatusV1::Ok);
        assert_eq!(result_status(&completed), "authorized");
        let envelope = sidecar_bytes(&completed, "authority_envelope");
        let receipt = sidecar_bytes(&completed, "durable_receipt");
        let audit_after_completion = recovered
            .provenance()
            .expect("completed recovery provenance");
        let stored: BTreeMap<[u8; 32], StoredAuthorizationV1> =
            load_canonical_records(&state_directory(parent.path()).join("authority-receipts-v1"))
                .expect("load recovered generic authorization");
        let stored = stored
            .get(&fixture.operation_id)
            .expect("recovered generic authorization record");
        assert_eq!(
            stored.durable_receipt.commit_sequence,
            stored.envelope_receipt.commit_sequence + 1,
            "generic signer commits were not adjacent after {phase:?}"
        );

        let exact_retry = authorize(
            &recovered,
            &fixture.request_json(),
            Vec::new(),
            TEST_NOW_MILLIS_V1 + 60_002,
        )
        .expect("retry completed recovered authorization");
        assert_eq!(exact_retry.status, OperationStatusV1::Replayed);
        assert_eq!(sidecar_bytes(&exact_retry, "authority_envelope"), envelope);
        assert_eq!(sidecar_bytes(&exact_retry, "durable_receipt"), receipt);
        assert_eq!(
            recovered.provenance().expect("exact retry provenance"),
            audit_after_completion,
            "exact retry appended an audit record after {phase:?}"
        );

        let conflict_path = create_artifact(parent.path(), "conflicting-reuse.bin", b"conflict");
        let conflicting = ClientRequestFixtureV1::new(
            TairaAuthorityRoleV1::RolloutObservation,
            case,
            &[("conflicting-reuse.bin", b"conflict")],
        );
        assert_eq!(conflicting.run_id, fixture.run_id);
        assert_ne!(conflicting.operation_id, fixture.operation_id);
        assert_eq!(
            authorize(
                &recovered,
                &conflicting.request_json(),
                read_only_descriptors(&[&conflict_path]),
                TEST_NOW_MILLIS_V1 + 60_003,
            ),
            Err(TairaAuthorityErrorV1::Conflict),
            "conflicting replay was accepted after {phase:?}"
        );
    }
}

#[test]
#[expect(clippy::too_many_lines, reason = "cohesive assignment conflict matrix")]
fn assignment_conflicts_and_run_windows_fail_closed() {
    let parent = temporary_parent();
    let service = provision(parent.path(), TairaAuthorityRoleV1::RolloutObservation);
    let active = ClientRequestFixtureV1::new(
        TairaAuthorityRoleV1::RolloutObservation,
        "assignment-reuse",
        &[("result.json", b"result")],
    );

    let future_issued = active.assignment_json(
        &service,
        TEST_NOW_MILLIS_V1 + 1,
        TEST_NOW_MILLIS_V1 + 1,
        TEST_NOW_MILLIS_V1 + 100,
    );
    assert_eq!(
        service.assign_run_json(&future_issued, TEST_NOW_MILLIS_V1),
        Err(TairaAuthorityErrorV1::Rejected)
    );

    let active_assignment = active.assignment_json(
        &service,
        TEST_NOW_MILLIS_V1 - 10,
        TEST_NOW_MILLIS_V1 - 1,
        TEST_NOW_MILLIS_V1 + 100,
    );
    assert_eq!(
        service
            .assign_run_json(&active_assignment, TEST_NOW_MILLIS_V1)
            .expect("active assignment")
            .status,
        OperationStatusV1::Ok
    );
    assert_eq!(
        service
            .assign_run_json(&active_assignment, TEST_NOW_MILLIS_V1)
            .expect("assignment retry")
            .status,
        OperationStatusV1::Replayed
    );
    let mut conflicting = parse_json(&active_assignment);
    conflicting
        .as_object_mut()
        .expect("assignment object")
        .insert("subject_sha256".into(), Value::from("91".repeat(32)));
    assert_eq!(
        service.assign_run_json(&canonical_json_line(&conflicting), TEST_NOW_MILLIS_V1),
        Err(TairaAuthorityErrorV1::Conflict)
    );

    let future = ClientRequestFixtureV1::new(
        TairaAuthorityRoleV1::RolloutObservation,
        "future-run",
        &[("future.json", b"future")],
    );
    service
        .assign_run_json(
            &future.assignment_json(
                &service,
                TEST_NOW_MILLIS_V1 - 10,
                TEST_NOW_MILLIS_V1 + 10,
                TEST_NOW_MILLIS_V1 + 100,
            ),
            TEST_NOW_MILLIS_V1,
        )
        .expect("future-not-before assignment");
    let future_artifact = create_artifact(parent.path(), "future.json", b"future");
    assert_eq!(
        authorize(
            &service,
            &future.request_json(),
            read_only_descriptors(&[&future_artifact]),
            TEST_NOW_MILLIS_V1,
        ),
        Err(TairaAuthorityErrorV1::Rejected)
    );

    let stale = ClientRequestFixtureV1::new(
        TairaAuthorityRoleV1::RolloutObservation,
        "stale-run",
        &[("stale.json", b"stale")],
    );
    service
        .assign_run_json(
            &stale.assignment_json(
                &service,
                TEST_NOW_MILLIS_V1 - 20,
                TEST_NOW_MILLIS_V1 - 10,
                TEST_NOW_MILLIS_V1,
            ),
            TEST_NOW_MILLIS_V1,
        )
        .expect("expired assignment is retained for historical audit");
    let stale_artifact = create_artifact(parent.path(), "stale.json", b"stale");
    assert_eq!(
        authorize(
            &service,
            &stale.request_json(),
            read_only_descriptors(&[&stale_artifact]),
            TEST_NOW_MILLIS_V1,
        ),
        Err(TairaAuthorityErrorV1::Rejected)
    );

    let too_long = ClientRequestFixtureV1::new(
        TairaAuthorityRoleV1::RolloutObservation,
        "too-long-run",
        &[("too-long.json", b"too-long")],
    );
    assert_eq!(
        service.assign_run_json(
            &too_long.assignment_json(
                &service,
                TEST_NOW_MILLIS_V1,
                TEST_NOW_MILLIS_V1,
                TEST_NOW_MILLIS_V1 + 24 * 60 * 60 * 1_000 + 1,
            ),
            TEST_NOW_MILLIS_V1,
        ),
        Err(TairaAuthorityErrorV1::Rejected)
    );
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "native controller identity regression"
)]
fn native_assignment_authenticates_controller_identity_and_unique_run_nonce() {
    let parent = temporary_parent();
    let service = provision(parent.path(), TairaAuthorityRoleV1::NativeEvidence);
    let fixture = ClientRequestFixtureV1::new(
        TairaAuthorityRoleV1::NativeEvidence,
        "native-controller-assignment",
        &[],
    );
    let assignment = fixture.assignment_json(
        &service,
        TEST_NOW_MILLIS_V1 - 10,
        TEST_NOW_MILLIS_V1 - 1,
        TEST_NOW_MILLIS_V1 + 60_000,
    );
    let assigned = service
        .assign_run_json(&assignment, TEST_NOW_MILLIS_V1)
        .expect("assign native controller-bound run");
    assert_eq!(assigned.status, OperationStatusV1::Ok);
    let assigned_result = parse_json(&assigned.result_json);
    let signed_assignment = assigned_result
        .get("assignment")
        .and_then(Value::as_object)
        .expect("signed native assignment");
    assert_eq!(
        signed_assignment.get("controller_digest"),
        Some(&Value::from(hex::encode(TEST_NATIVE_CONTROLLER_DIGEST_V1)))
    );
    assert_eq!(
        signed_assignment.get("controller_host_id"),
        Some(&Value::from(TEST_NATIVE_CONTROLLER_HOST_ID_V1))
    );
    assert_eq!(
        signed_assignment.get("controller_installation_id"),
        Some(&Value::from(TEST_NATIVE_CONTROLLER_INSTALLATION_ID_V1))
    );
    assert_eq!(
        signed_assignment.get("run_nonce"),
        Some(&Value::from(hex::encode(TEST_NATIVE_RUN_NONCE_V1)))
    );
    assert_eq!(
        service
            .assign_run_json(&assignment, TEST_NOW_MILLIS_V1)
            .expect("retry exact native assignment")
            .status,
        OperationStatusV1::Replayed
    );

    let second = ClientRequestFixtureV1::new(
        TairaAuthorityRoleV1::NativeEvidence,
        "native-controller-assignment-second-run",
        &[],
    );
    assert_eq!(
        service.assign_run_json(
            &second.assignment_json(
                &service,
                TEST_NOW_MILLIS_V1 - 10,
                TEST_NOW_MILLIS_V1 - 1,
                TEST_NOW_MILLIS_V1 + 60_000,
            ),
            TEST_NOW_MILLIS_V1,
        ),
        Err(TairaAuthorityErrorV1::Conflict)
    );

    let reject_field = |field: &str, replacement: Option<Value>| {
        let parent = temporary_parent();
        let service = provision(parent.path(), TairaAuthorityRoleV1::NativeEvidence);
        let fixture = ClientRequestFixtureV1::new(
            TairaAuthorityRoleV1::NativeEvidence,
            "invalid-native-controller-assignment",
            &[],
        );
        let mut assignment = parse_json(&fixture.assignment_json(
            &service,
            TEST_NOW_MILLIS_V1 - 10,
            TEST_NOW_MILLIS_V1 - 1,
            TEST_NOW_MILLIS_V1 + 60_000,
        ));
        let assignment = assignment
            .as_object_mut()
            .expect("native assignment object");
        if let Some(replacement) = replacement {
            assignment.insert(field.to_owned(), replacement);
        } else {
            assignment.remove(field);
        }
        assert_eq!(
            service.assign_run_json(
                &canonical_json_line(&Value::Object(assignment.clone())),
                TEST_NOW_MILLIS_V1,
            ),
            Err(TairaAuthorityErrorV1::Rejected),
            "invalid native assignment field was accepted: {field}"
        );
    };
    for field in [
        "controller_digest",
        "controller_host_id",
        "controller_installation_id",
        "run_nonce",
    ] {
        reject_field(field, None);
    }
    reject_field("controller_digest", Some(Value::from("00".repeat(32))));
    reject_field("controller_host_id", Some(Value::from("Uppercase-host")));
    reject_field(
        "controller_installation_id",
        Some(Value::from("installation/with/path")),
    );
    reject_field("run_nonce", Some(Value::from("00".repeat(32))));

    let other_parent = temporary_parent();
    let other_service = provision(
        other_parent.path(),
        TairaAuthorityRoleV1::RolloutObservation,
    );
    let other = ClientRequestFixtureV1::new(
        TairaAuthorityRoleV1::RolloutObservation,
        "foreign-controller-binding",
        &[],
    );
    let mut other_assignment = parse_json(&other.assignment_json(
        &other_service,
        TEST_NOW_MILLIS_V1 - 10,
        TEST_NOW_MILLIS_V1 - 1,
        TEST_NOW_MILLIS_V1 + 60_000,
    ));
    other_assignment
        .as_object_mut()
        .expect("other assignment object")
        .insert(
            "controller_digest".into(),
            Value::from(hex::encode(TEST_NATIVE_CONTROLLER_DIGEST_V1)),
        );
    assert_eq!(
        other_service.assign_run_json(&canonical_json_line(&other_assignment), TEST_NOW_MILLIS_V1,),
        Err(TairaAuthorityErrorV1::Rejected)
    );
}

fn mutate_object_path(value: &mut Value, path: &[&str]) {
    let (leaf, parents) = path.split_last().expect("nonempty mutation path");
    let mut current = value;
    for field in parents {
        current = current
            .as_object_mut()
            .and_then(|object| object.get_mut(*field))
            .expect("mutation parent field");
    }
    let value = current
        .as_object_mut()
        .and_then(|object| object.get_mut(*leaf))
        .expect("mutation leaf field");
    let replacement = match (value.as_str(), value.as_u64()) {
        (Some(text), _) => Value::from(format!("{text}-mutated")),
        (_, Some(number)) => Value::from(number.checked_add(1).expect("fixture integer mutation")),
        _ => panic!("unsupported mutation scalar at {path:?}"),
    };
    *value = replacement;
}

#[test]
#[expect(clippy::too_many_lines, reason = "signed sidecar binding regression")]
fn native_signed_sidecars_bind_controller_claims_and_exact_generic_receipt() {
    let parent = temporary_parent();
    let native = super::native_evidence::tests::authority_service_fixture();
    let fixture = ClientRequestFixtureV1::from_subject_and_manifest(
        TairaAuthorityRoleV1::NativeEvidence,
        native.subject.clone(),
        native.manifest.clone(),
    );
    let service = provision(parent.path(), TairaAuthorityRoleV1::NativeEvidence);
    assign_active_run(&service, &fixture);
    let paths = native
        .paths
        .iter()
        .map(PathBuf::as_path)
        .collect::<Vec<_>>();
    authorize(
        &service,
        &fixture.request_json(),
        read_only_descriptors(&paths),
        TEST_NOW_MILLIS_V1,
    )
    .expect("authorize native sidecar fixture");

    let records: BTreeMap<[u8; 32], StoredAuthorizationV1> =
        load_canonical_records(&state_directory(parent.path()).join("authority-receipts-v1"))
            .expect("load native authorization record");
    let stored = records
        .get(&fixture.operation_id)
        .expect("native authorization record");
    service
        .verify_stored_authorization_for_test(stored)
        .expect("valid native signed sidecars");

    let envelope = parse_json(&stored.authority_envelope_json);
    let claims = envelope
        .get("claims")
        .and_then(Value::as_object)
        .expect("native envelope claims");
    assert_eq!(
        claims.get("controller_digest"),
        Some(&Value::from(hex::encode(TEST_NATIVE_CONTROLLER_DIGEST_V1)))
    );
    assert_eq!(
        claims.get("controller_host_id"),
        Some(&Value::from(TEST_NATIVE_CONTROLLER_HOST_ID_V1))
    );
    assert_eq!(
        claims.get("controller_installation_id"),
        Some(&Value::from(TEST_NATIVE_CONTROLLER_INSTALLATION_ID_V1))
    );
    assert_eq!(
        claims.get("run_nonce"),
        Some(&Value::from(hex::encode(TEST_NATIVE_RUN_NONCE_V1)))
    );

    for field in [
        "controller_digest",
        "controller_host_id",
        "controller_installation_id",
        "run_nonce",
    ] {
        let mut mutated = stored.clone();
        let mut envelope = parse_json(&mutated.authority_envelope_json);
        mutate_object_path(&mut envelope, &["claims", field]);
        mutated.authority_envelope_json = canonical_json_line(&envelope);
        assert_eq!(
            service.verify_stored_authorization_for_test(&mutated),
            Err(TairaAuthorityErrorV1::State),
            "mutated native envelope claim was accepted: {field}"
        );
    }

    let receipt_fields: &[&[&str]] = &[
        &["schema"],
        &["schema_version"],
        &["role"],
        &["signature_algorithm"],
        &["binding_sha256"],
        &["signature"],
        &["audit_sequence"],
        &["audit_head"],
        &["claims", "schema"],
        &["claims", "role"],
        &["claims", "decision"],
        &["claims", "replay_namespace"],
        &["claims", "operation_id"],
        &["claims", "run_id"],
        &["claims", "subject_sha256"],
        &["claims", "authority_envelope_sha256"],
        &["claims", "admitted_at_unix_millis"],
        &["claims", "authority_audit_sequence"],
        &["claims", "authority_audit_head"],
    ];
    for path in receipt_fields {
        let mut mutated = stored.clone();
        let mut receipt = parse_json(&mutated.durable_receipt_json);
        mutate_object_path(&mut receipt, path);
        mutated.durable_receipt_json = canonical_json_line(&receipt);
        assert_eq!(
            service.verify_stored_authorization_for_test(&mutated),
            Err(TairaAuthorityErrorV1::State),
            "mutated generic durable receipt field was accepted: {path:?}"
        );
    }

    let mut mutated = stored.clone();
    let mut receipt = parse_json(&mutated.durable_receipt_json);
    receipt
        .as_object_mut()
        .expect("generic durable receipt object")
        .insert("unexpected".into(), Value::from(true));
    mutated.durable_receipt_json = canonical_json_line(&receipt);
    assert_eq!(
        service.verify_stored_authorization_for_test(&mutated),
        Err(TairaAuthorityErrorV1::State)
    );
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "descriptor integrity mutation matrix"
)]
fn descriptor_alias_mutability_identity_and_hash_drift_are_rejected() {
    let parent = temporary_parent();
    let service = provision(parent.path(), TairaAuthorityRoleV1::RolloutObservation);

    let aliased_path = create_artifact(parent.path(), "aliased.bin", b"same");
    let aliased = ClientRequestFixtureV1::new(
        TairaAuthorityRoleV1::RolloutObservation,
        "descriptor-alias",
        &[("first.bin", b"same"), ("second.bin", b"same")],
    );
    assign_active_run(&service, &aliased);
    assert_eq!(
        authorize(
            &service,
            &aliased.request_json(),
            read_only_descriptors(&[&aliased_path, &aliased_path]),
            TEST_NOW_MILLIS_V1,
        ),
        Err(TairaAuthorityErrorV1::Rejected)
    );

    let hardlink_path = create_artifact(parent.path(), "hardlink.bin", b"hardlink");
    fs::hard_link(&hardlink_path, parent.path().join("hardlink-copy.bin"))
        .expect("create hardlink alias");
    let hardlink = ClientRequestFixtureV1::new(
        TairaAuthorityRoleV1::RolloutObservation,
        "hardlink",
        &[("hardlink.bin", b"hardlink")],
    );
    assign_active_run(&service, &hardlink);
    assert_eq!(
        authorize(
            &service,
            &hardlink.request_json(),
            read_only_descriptors(&[&hardlink_path]),
            TEST_NOW_MILLIS_V1,
        ),
        Err(TairaAuthorityErrorV1::Rejected)
    );

    let writable_path = create_artifact(parent.path(), "writable.bin", b"writable");
    fs::set_permissions(&writable_path, fs::Permissions::from_mode(0o600))
        .expect("make writable descriptor fixture writable");
    let writable = ClientRequestFixtureV1::new(
        TairaAuthorityRoleV1::RolloutObservation,
        "writable-descriptor",
        &[("writable.bin", b"writable")],
    );
    assign_active_run(&service, &writable);
    let writable_fd = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&writable_path)
        .expect("open writable descriptor");
    assert_eq!(
        authorize(
            &service,
            &writable.request_json(),
            vec![OwnedFd::from(writable_fd)],
            TEST_NOW_MILLIS_V1,
        ),
        Err(TairaAuthorityErrorV1::Rejected)
    );

    let mutable_path = create_artifact(parent.path(), "mutable.bin", b"mutable");
    fs::set_permissions(&mutable_path, fs::Permissions::from_mode(0o620))
        .expect("make artifact group-writable");
    let mutable = ClientRequestFixtureV1::new(
        TairaAuthorityRoleV1::RolloutObservation,
        "mutable-mode",
        &[("mutable.bin", b"mutable")],
    );
    assign_active_run(&service, &mutable);
    assert_eq!(
        authorize(
            &service,
            &mutable.request_json(),
            read_only_descriptors(&[&mutable_path]),
            TEST_NOW_MILLIS_V1,
        ),
        Err(TairaAuthorityErrorV1::Rejected)
    );

    let drift_path = create_artifact(parent.path(), "drift.bin", b"trusted");
    let drift = ClientRequestFixtureV1::new(
        TairaAuthorityRoleV1::RolloutObservation,
        "hash-drift",
        &[("drift.bin", b"trusted")],
    );
    assign_active_run(&service, &drift);
    fs::set_permissions(&drift_path, fs::Permissions::from_mode(0o600))
        .expect("make drift fixture writable");
    let mut mutator = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(&drift_path)
        .expect("open drift mutator");
    mutator.write_all(b"changed").expect("mutate artifact");
    mutator.sync_all().expect("sync artifact mutation");
    drop(mutator);
    fs::set_permissions(&drift_path, fs::Permissions::from_mode(0o400))
        .expect("restore immutable drift fixture mode");
    assert_eq!(
        authorize(
            &service,
            &drift.request_json(),
            read_only_descriptors(&[&drift_path]),
            TEST_NOW_MILLIS_V1,
        ),
        Err(TairaAuthorityErrorV1::Rejected)
    );

    let uid_path = create_artifact(parent.path(), "uid.bin", b"uid");
    let uid = ClientRequestFixtureV1::new(
        TairaAuthorityRoleV1::RolloutObservation,
        "peer-uid",
        &[("uid.bin", b"uid")],
    );
    assign_active_run(&service, &uid);
    let wrong_uid = service
        .public_binding()
        .expect("peer UID binding")
        .signer
        .client_uid
        .checked_add(1)
        .expect("distinct fixture UID");
    assert_eq!(
        service.authorize_json(
            &uid.request_json(),
            read_only_descriptors(&[&uid_path]),
            wrong_uid,
            TEST_NOW_MILLIS_V1,
        ),
        Err(TairaAuthorityErrorV1::Rejected)
    );
}

#[test]
fn artifact_immutability_requires_root_or_service_ownership_and_no_write_bits() {
    let service_uid = 48_001;
    let client_uid = 48_002;
    assert!(artifact_is_authority_immutable(0, 0o400, service_uid));
    assert!(artifact_is_authority_immutable(
        service_uid,
        0o440,
        service_uid
    ));
    assert!(!artifact_is_authority_immutable(
        client_uid,
        0o400,
        service_uid
    ));
    for mode in [0o600, 0o420, 0o402] {
        assert!(!artifact_is_authority_immutable(
            service_uid,
            mode,
            service_uid
        ));
    }
}

fn replay_record(case: u8) -> ReplayConsumptionV1 {
    ReplayConsumptionV1 {
        run_id: [case; 32],
        operation_id: [case.wrapping_add(1); 32],
        request_sha256: [case.wrapping_add(2); 32],
        subject_sha256: [case.wrapping_add(3); 32],
        artifact_manifest_sha256: [case.wrapping_add(4); 32],
        consumed_at_unix_millis: TEST_NOW_MILLIS_V1 + u64::from(case),
    }
}

#[test]
fn public_soak_binding_anchor_is_write_ahead_signed_and_open_stable() {
    let parent = temporary_parent();
    let state = public_soak_anchor_state_directory(parent.path());
    let (service, input, anchor) = provision_public_soak_anchor_fixture(parent.path());
    assert_eq!(input.operation_id, anchor.operation_id);
    assert_eq!(input.replay_binding, anchor.replay_binding);
    assert_eq!(input.observation_binding, anchor.observation_binding);
    assert_eq!(input.signing_payload, anchor.signing_payload);
    assert_eq!(anchor.receipt.commit_sequence, 2);
    assert_eq!(
        anchor.previous_audit_head,
        input.replay_binding.signer.audit_genesis_digest
    );

    let provisioned = service.provenance().expect("provisioned anchor provenance");
    assert_eq!(provisioned.audit_sequence, 2);
    service
        .ensure_public_soak_observation_binding_anchor_for_test()
        .expect("exact anchor retry");
    assert_eq!(
        service.provenance().expect("retried anchor provenance"),
        provisioned,
        "an exact anchor retry must not append"
    );
    drop(service);

    let reopened = TairaAuthorityServiceV1::open(&state, wrapping_key())
        .expect("open signed observation binding anchor");
    assert_eq!(
        reopened.provenance().expect("reopened anchor provenance"),
        provisioned,
        "open must verify the persisted anchor without signing"
    );
    let reopened_inputs: BTreeMap<[u8; 32], StoredPublicSoakObservationBindingInputV1> =
        load_canonical_records(&state.join("public-soak-observation-binding-input-v1"))
            .expect("reload observation binding input");
    let reopened_anchors: BTreeMap<[u8; 32], StoredPublicSoakObservationBindingAnchorV1> =
        load_canonical_records(&state.join("public-soak-observation-binding-v1"))
            .expect("reload observation binding anchor");
    assert_eq!(
        reopened_inputs,
        BTreeMap::from([(input.operation_id, input)])
    );
    assert_eq!(
        reopened_anchors,
        BTreeMap::from([(anchor.operation_id, anchor)])
    );
}

#[test]
fn public_soak_binding_anchor_recovers_both_write_ahead_crash_phases() {
    for phase in [
        PublicSoakBindingCrashPhaseV1::AfterInputPersistence,
        PublicSoakBindingCrashPhaseV1::AfterSignerCommit,
    ] {
        let parent = temporary_parent();
        let state = public_soak_anchor_state_directory(parent.path());
        let observation_binding = independent_public_soak_observation_binding(parent.path());
        assert!(matches!(
            TairaAuthorityServiceV1::provision_with_public_soak_observation_binding_crash_for_test(
                &state,
                provisioning(TairaAuthorityRoleV1::PublicSoakReplayAdmission),
                wrapping_key(),
                observation_binding,
                phase,
            ),
            Err(TairaAuthorityErrorV1::State)
        ));
        let inputs: BTreeMap<[u8; 32], StoredPublicSoakObservationBindingInputV1> =
            load_canonical_records(&state.join("public-soak-observation-binding-input-v1"))
                .expect("load crash-persisted binding input");
        assert_eq!(inputs.len(), 1);
        let anchors: BTreeMap<[u8; 32], StoredPublicSoakObservationBindingAnchorV1> =
            load_canonical_records(&state.join("public-soak-observation-binding-v1"))
                .expect("load absent crash anchor directory");
        assert!(anchors.is_empty());
        let committed_audit_path = audit_record_path(&state, 2);
        let committed_audit_before_open =
            if phase == PublicSoakBindingCrashPhaseV1::AfterSignerCommit {
                Some(fs::read(&committed_audit_path).expect("read exact pre-crash signer commit"))
            } else {
                assert!(!committed_audit_path.exists());
                None
            };

        let recovered = TairaAuthorityServiceV1::open(&state, wrapping_key())
            .expect("recover observation binding anchor from write-ahead input");
        let recovered_provenance = recovered
            .provenance()
            .expect("recovered binding anchor provenance");
        assert_eq!(recovered_provenance.audit_sequence, 2);
        if let Some(committed_audit_before_open) = committed_audit_before_open {
            assert_eq!(
                fs::read(&committed_audit_path).expect("read recovered exact signer commit"),
                committed_audit_before_open,
                "recovery after the signer commit must replay it without mutation"
            );
        }
        let anchor_path = state.join("public-soak-observation-binding-v1");
        let recovered_anchors: BTreeMap<[u8; 32], StoredPublicSoakObservationBindingAnchorV1> =
            load_canonical_records(&anchor_path).expect("load recovered binding anchor");
        assert_eq!(recovered_anchors.len(), 1);
        let recovered_anchor_bytes = fs::read(record_path(
            &anchor_path,
            recovered_anchors
                .values()
                .next()
                .expect("recovered binding anchor")
                .operation_id,
        ))
        .expect("read recovered binding anchor bytes");
        recovered
            .ensure_public_soak_observation_binding_anchor_for_test()
            .expect("exact recovered anchor retry");
        assert_eq!(
            recovered
                .provenance()
                .expect("exact recovered anchor retry provenance"),
            recovered_provenance
        );
        drop(recovered);

        let reopened = TairaAuthorityServiceV1::open(&state, wrapping_key())
            .expect("reopen recovered binding anchor");
        assert_eq!(
            reopened
                .provenance()
                .expect("reopened recovered anchor provenance"),
            recovered_provenance
        );
        assert_eq!(
            fs::read(record_path(
                &anchor_path,
                recovered_anchors
                    .values()
                    .next()
                    .expect("stable recovered binding anchor")
                    .operation_id,
            ))
            .expect("reread recovered binding anchor bytes"),
            recovered_anchor_bytes,
            "subsequent opens must preserve byte-identical anchor state"
        );
    }
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "complete binding and receipt mutation matrix"
)]
fn public_soak_binding_anchor_rejects_full_binding_and_receipt_mutations() {
    let parent = temporary_parent();
    let (service, input, anchor) = provision_public_soak_anchor_fixture(parent.path());

    macro_rules! reject_input_mutation {
        ($label:literal, $mutation:expr) => {{
            let mut mutated = input.clone();
            ($mutation)(&mut mutated);
            assert!(
                service
                    .verify_public_soak_observation_binding_anchor_for_test(&mutated, &anchor)
                    .is_err(),
                "write-ahead input mutation was accepted: {}",
                $label
            );
        }};
    }
    reject_input_mutation!(
        "operation_id",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.operation_id[0] ^= 1;
        }
    );
    reject_input_mutation!(
        "replay outer magic",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.replay_binding.magic[0] ^= 1;
        }
    );
    reject_input_mutation!(
        "replay outer version",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.replay_binding.version += 1;
        }
    );
    reject_input_mutation!(
        "replay outer role",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.replay_binding.role = TairaAuthorityRoleV1::PublicSoakObservation;
        }
    );
    reject_input_mutation!(
        "replay signer magic",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.replay_binding.signer.magic[0] ^= 1;
        }
    );
    reject_input_mutation!(
        "replay signer version",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.replay_binding.signer.version += 1;
        }
    );
    reject_input_mutation!(
        "replay handle",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.replay_binding.signer.handle.push_str("-substituted");
        }
    );
    reject_input_mutation!(
        "replay service identity",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value
                .replay_binding
                .signer
                .service_id
                .push_str("-substituted");
        }
    );
    reject_input_mutation!(
        "replay administrator identity",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value
                .replay_binding
                .signer
                .administrator_id
                .push_str("-substituted");
        }
    );
    reject_input_mutation!(
        "replay service UID",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.replay_binding.signer.service_uid += 100;
        }
    );
    reject_input_mutation!(
        "replay client UID",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.replay_binding.signer.client_uid += 101;
        }
    );
    reject_input_mutation!(
        "replay administrator UID",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.replay_binding.signer.administrator_uid += 102;
        }
    );
    reject_input_mutation!(
        "replay signer role",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.replay_binding.signer.role = SoftwareSignerRoleV1::EvidenceViewer;
        }
    );
    reject_input_mutation!(
        "replay purpose binding",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            let SoftwareSignerPurposeBindingV1::TairaAuthority { role } =
                &mut value.replay_binding.signer.purpose_binding
            else {
                unreachable!("Taira fixture purpose binding")
            };
            role.push_str("-substituted");
        }
    );
    reject_input_mutation!(
        "replay signing domain",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.replay_binding.signer.domain.push_str("-substituted");
        }
    );
    reject_input_mutation!(
        "replay key algorithm",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.replay_binding.signer.key_algorithm = SoftwareSignerKeyAlgorithmV1::MlDsa;
        }
    );
    reject_input_mutation!(
        "replay key revision",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.replay_binding.signer.key_revision += 1;
        }
    );
    reject_input_mutation!(
        "replay policy revision",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.replay_binding.signer.policy_revision += 1;
        }
    );
    reject_input_mutation!(
        "replay policy digest",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.replay_binding.signer.policy_digest[0] ^= 1;
        }
    );
    reject_input_mutation!(
        "replay public key",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.replay_binding.signer.public_key =
                KeyPair::try_random_with_algorithm(Algorithm::Ed25519)
                    .expect("replacement public key")
                    .public_key()
                    .clone();
        }
    );
    reject_input_mutation!(
        "replay public key digest",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.replay_binding.signer.public_key_digest[0] ^= 1;
        }
    );
    reject_input_mutation!(
        "replay audit genesis",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.replay_binding.signer.audit_genesis_digest[0] ^= 1;
        }
    );
    reject_input_mutation!(
        "replay maximum request bytes",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.replay_binding.signer.max_request_bytes -= 1;
        }
    );
    reject_input_mutation!(
        "observation service identity",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value
                .observation_binding
                .signer
                .service_id
                .push_str("-substituted");
        }
    );
    reject_input_mutation!(
        "observation administrator identity",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value
                .observation_binding
                .signer
                .administrator_id
                .push_str("-substituted");
        }
    );
    reject_input_mutation!(
        "observation service UID",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.observation_binding.signer.service_uid += 100;
        }
    );
    reject_input_mutation!(
        "observation client UID",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.observation_binding.signer.client_uid += 101;
        }
    );
    reject_input_mutation!(
        "observation administrator UID",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.observation_binding.signer.administrator_uid += 102;
        }
    );
    reject_input_mutation!(
        "observation key revision",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.observation_binding.signer.key_revision += 1;
        }
    );
    reject_input_mutation!(
        "observation policy revision",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.observation_binding.signer.policy_revision += 1;
        }
    );
    reject_input_mutation!(
        "observation policy digest",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.observation_binding.signer.policy_digest[0] ^= 1;
        }
    );
    reject_input_mutation!(
        "observation public key digest",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.observation_binding.signer.public_key_digest[0] ^= 1;
        }
    );
    reject_input_mutation!(
        "replay binding digest",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.replay_binding_sha256[0] ^= 1;
        }
    );
    reject_input_mutation!(
        "observation binding digest",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.observation_binding_sha256[0] ^= 1;
        }
    );
    reject_input_mutation!(
        "signing payload",
        |value: &mut StoredPublicSoakObservationBindingInputV1| {
            value.signing_payload[0] ^= 1;
        }
    );

    macro_rules! reject_anchor_mutation {
        ($label:literal, $mutation:expr) => {{
            let mut mutated = anchor.clone();
            ($mutation)(&mut mutated);
            assert!(
                service
                    .verify_public_soak_observation_binding_anchor_for_test(&input, &mutated)
                    .is_err(),
                "signed anchor mutation was accepted: {}",
                $label
            );
        }};
    }
    reject_anchor_mutation!(
        "operation ID",
        |value: &mut StoredPublicSoakObservationBindingAnchorV1| {
            value.operation_id[0] ^= 1;
        }
    );
    reject_anchor_mutation!(
        "replay binding",
        |value: &mut StoredPublicSoakObservationBindingAnchorV1| {
            value.replay_binding.signer.policy_revision += 1;
        }
    );
    reject_anchor_mutation!(
        "replay binding digest",
        |value: &mut StoredPublicSoakObservationBindingAnchorV1| {
            value.replay_binding_sha256[0] ^= 1;
        }
    );
    reject_anchor_mutation!(
        "observation binding",
        |value: &mut StoredPublicSoakObservationBindingAnchorV1| {
            value.observation_binding.signer.policy_digest[0] ^= 1;
        }
    );
    reject_anchor_mutation!(
        "observation binding digest",
        |value: &mut StoredPublicSoakObservationBindingAnchorV1| {
            value.observation_binding_sha256[0] ^= 1;
        }
    );
    reject_anchor_mutation!(
        "audit predecessor",
        |value: &mut StoredPublicSoakObservationBindingAnchorV1| {
            value.previous_audit_head[0] ^= 1;
        }
    );
    reject_anchor_mutation!(
        "signing payload",
        |value: &mut StoredPublicSoakObservationBindingAnchorV1| {
            value.signing_payload[0] ^= 1;
        }
    );
    reject_anchor_mutation!(
        "receipt operation",
        |value: &mut StoredPublicSoakObservationBindingAnchorV1| {
            value.receipt.operation_id[0] ^= 1;
        }
    );
    reject_anchor_mutation!(
        "receipt request digest",
        |value: &mut StoredPublicSoakObservationBindingAnchorV1| {
            value.receipt.request_digest[0] ^= 1;
        }
    );
    reject_anchor_mutation!(
        "receipt payload digest",
        |value: &mut StoredPublicSoakObservationBindingAnchorV1| {
            value.receipt.payload_digest[0] ^= 1;
        }
    );
    reject_anchor_mutation!(
        "receipt payload length",
        |value: &mut StoredPublicSoakObservationBindingAnchorV1| {
            value.receipt.payload_length += 1;
        }
    );
    reject_anchor_mutation!(
        "receipt signature",
        |value: &mut StoredPublicSoakObservationBindingAnchorV1| {
            value.receipt.signature[0] ^= 1;
        }
    );
    reject_anchor_mutation!(
        "receipt sequence",
        |value: &mut StoredPublicSoakObservationBindingAnchorV1| {
            value.receipt.commit_sequence += 1;
        }
    );
    reject_anchor_mutation!(
        "receipt audit head",
        |value: &mut StoredPublicSoakObservationBindingAnchorV1| {
            value.receipt.commit_audit_head[0] ^= 1;
        }
    );
    reject_anchor_mutation!(
        "receipt replay status",
        |value: &mut StoredPublicSoakObservationBindingAnchorV1| {
            value.receipt.replayed = !value.receipt.replayed;
        }
    );
    reject_anchor_mutation!(
        "receipt provenance",
        |value: &mut StoredPublicSoakObservationBindingAnchorV1| {
            value.receipt.provenance.audit_head[0] ^= 1;
        }
    );
    reject_anchor_mutation!(
        "receipt response digest",
        |value: &mut StoredPublicSoakObservationBindingAnchorV1| {
            value.receipt.response_digest[0] ^= 1;
        }
    );
    reject_anchor_mutation!(
        "receipt response attestation",
        |value: &mut StoredPublicSoakObservationBindingAnchorV1| {
            value.receipt.response_attestation[0] ^= 1;
        }
    );
}

#[test]
fn public_soak_binding_anchor_open_rejects_policy_substitution() {
    for mutate_input in [true, false] {
        let parent = temporary_parent();
        let state = public_soak_anchor_state_directory(parent.path());
        let (service, input, anchor) = provision_public_soak_anchor_fixture(parent.path());
        drop(service);
        if mutate_input {
            let directory = state.join("public-soak-observation-binding-input-v1");
            fs::remove_file(record_path(&directory, input.operation_id))
                .expect("remove valid binding input");
            let mut substituted = input.clone();
            substituted.observation_binding.signer.policy_revision += 1;
            persist_canonical_once(&directory, input.operation_id, &substituted)
                .expect("persist substituted binding input");
        } else {
            let directory = state.join("public-soak-observation-binding-v1");
            fs::remove_file(record_path(&directory, anchor.operation_id))
                .expect("remove valid signed anchor");
            let mut substituted = anchor.clone();
            substituted.replay_binding.signer.policy_digest[0] ^= 1;
            persist_canonical_once(&directory, anchor.operation_id, &substituted)
                .expect("persist substituted signed anchor");
        }
        assert!(matches!(
            TairaAuthorityServiceV1::open(&state, wrapping_key()),
            Err(TairaAuthorityErrorV1::State)
        ));
    }
}

#[test]
fn canonical_store_recovers_pending_records_and_exact_retries() {
    let parent = temporary_parent();
    let records_directory = parent.path().join("records");
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&records_directory)
        .expect("create private records directory");
    let key = [0x31; 32];
    let record = replay_record(0x31);
    assert_eq!(
        persist_canonical_once(&records_directory, key, &record),
        Ok(PersistOutcomeV1::Fresh)
    );
    assert_eq!(
        persist_canonical_once(&records_directory, key, &record),
        Ok(PersistOutcomeV1::Existing)
    );

    let final_path = record_path(&records_directory, key);
    let pending_path = pending_record_path(&records_directory, key);
    fs::rename(&final_path, &pending_path).expect("simulate crash before pending promotion");
    let recovered: BTreeMap<[u8; 32], ReplayConsumptionV1> =
        load_canonical_records(&records_directory).expect("recover pending record");
    assert_eq!(recovered, BTreeMap::from([(key, record)]));
    assert!(final_path.is_file());
    assert!(!pending_path.exists());
}

#[test]
fn canonical_store_rejects_conflicting_pending_recovery() {
    let parent = temporary_parent();
    let records_directory = parent.path().join("records");
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&records_directory)
        .expect("create private records directory");
    let key = [0x41; 32];
    let accepted = replay_record(0x41);
    persist_canonical_once(&records_directory, key, &accepted).expect("persist accepted record");

    let conflicting = replay_record(0x42);
    let conflicting_bytes = norito::encode_canonical(&conflicting).expect("encode conflict");
    let mut pending = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(pending_record_path(&records_directory, key))
        .expect("create conflicting pending record");
    pending
        .write_all(&conflicting_bytes)
        .expect("write conflicting pending record");
    pending.sync_all().expect("sync conflicting pending record");
    drop(pending);

    assert!(
        load_canonical_records::<ReplayConsumptionV1>(&records_directory).is_err(),
        "recovery must not choose between two different durable values"
    );
}

#[test]
fn authority_open_recovers_a_complete_pending_audit_record() {
    let parent = temporary_parent();
    let service = provision(parent.path(), TairaAuthorityRoleV1::RolloutObservation);
    let fixture = ClientRequestFixtureV1::new(
        TairaAuthorityRoleV1::RolloutObservation,
        "pending-audit",
        &[("pending.json", b"pending")],
    );
    assign_active_run(&service, &fixture);
    let expected = service.provenance().expect("pre-crash provenance");
    assert_eq!(expected.audit_sequence, 2);
    drop(service);

    let state = state_directory(parent.path());
    let final_path = audit_record_path(&state, 2);
    let pending_path = pending_audit_record_path(&state, 2);
    fs::rename(&final_path, &pending_path).expect("simulate audit promotion crash");
    let recovered = TairaAuthorityServiceV1::open(&state, wrapping_key())
        .expect("recover complete pending audit record");
    assert_eq!(
        recovered.provenance().expect("recovered provenance"),
        expected
    );
    assert!(final_path.is_file());
    assert!(!pending_path.exists());
}

#[test]
fn authority_open_rejects_a_truncated_audit_record() {
    let parent = temporary_parent();
    let service = provision(parent.path(), TairaAuthorityRoleV1::RolloutObservation);
    let fixture = ClientRequestFixtureV1::new(
        TairaAuthorityRoleV1::RolloutObservation,
        "truncated-audit",
        &[("truncated.json", b"truncated")],
    );
    assign_active_run(&service, &fixture);
    drop(service);

    let state = state_directory(parent.path());
    let path = audit_record_path(&state, 2);
    let mut bytes = fs::read(&path).expect("read audit record");
    assert!(bytes.len() > 1);
    bytes.pop();
    let mut truncated = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(&path)
        .expect("open audit record for truncation");
    truncated.write_all(&bytes).expect("truncate audit record");
    truncated.sync_all().expect("sync truncated audit record");
    drop(truncated);

    assert_eq!(
        TairaAuthorityServiceV1::open(&state, wrapping_key()).unwrap_err(),
        TairaAuthorityErrorV1::State
    );
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "rotation and recovery security regression"
)]
fn rotation_invalidates_old_policy_and_revocation_survives_recovery() {
    let parent = temporary_parent();
    let service = provision(parent.path(), TairaAuthorityRoleV1::RolloutObservation);
    let artifact = create_artifact(parent.path(), "rotation.json", b"rotation");
    let fixture = ClientRequestFixtureV1::new(
        TairaAuthorityRoleV1::RolloutObservation,
        "rotation",
        &[("rotation.json", b"rotation")],
    );
    let old_assignment = fixture.assignment_json(
        &service,
        TEST_NOW_MILLIS_V1 - 10,
        TEST_NOW_MILLIS_V1 - 1,
        TEST_NOW_MILLIS_V1 + 100,
    );
    assert_eq!(
        service
            .assign_run_json(&old_assignment, TEST_NOW_MILLIS_V1)
            .expect("pre-rotation assignment")
            .status,
        OperationStatusV1::Ok
    );
    let old_authorization = authorize(
        &service,
        &fixture.request_json(),
        read_only_descriptors(&[&artifact]),
        TEST_NOW_MILLIS_V1,
    )
    .expect("pre-rotation authorization");
    let before = service.provenance().expect("pre-rotation provenance");

    assert_eq!(
        service
            .administer(
                AuthorityAdminCommandV1::Rotate {
                    operation_id: [0x61; 32],
                    expected_audit_head: [0xFF; 32],
                    expected_key_revision: 1,
                    new_key_revision: 2,
                    new_policy_revision: 2,
                    new_policy_digest: [0x62; 32],
                },
                TEST_NOW_MILLIS_V1,
            )
            .unwrap_err(),
        TairaAuthorityErrorV1::Conflict,
        "rotation must bind the exact predecessor"
    );
    assert_eq!(service.provenance().expect("unchanged provenance"), before);

    let rotate = AuthorityAdminCommandV1::Rotate {
        operation_id: [0x61; 32],
        expected_audit_head: before.audit_head,
        expected_key_revision: 1,
        new_key_revision: 2,
        new_policy_revision: 2,
        new_policy_digest: [0x62; 32],
    };
    let rotated = service
        .administer(rotate.clone(), TEST_NOW_MILLIS_V1)
        .expect("rotate authority");
    assert_eq!(rotated.status, OperationStatusV1::Ok);
    assert_eq!(result_status(&rotated), "successor-ready");
    assert_eq!(
        parse_json(&rotated.result_json)["schema"],
        Value::from("iroha.taira.authority-rotation-handoff.v1")
    );
    let after_rotation = service.provenance().expect("rotated provenance");
    assert_eq!(after_rotation.audit_sequence, before.audit_sequence + 1);
    assert_ne!(after_rotation.audit_head, before.audit_head);
    let binding = service.public_binding().expect("rotated binding");
    assert_eq!(binding.signer.key_revision, 2);
    assert_eq!(binding.signer.policy_revision, 2);
    assert_eq!(binding.signer.policy_digest, [0x62; 32]);

    let rotation_replay = service
        .administer(rotate, TEST_NOW_MILLIS_V1 + 1)
        .expect("exact rotation retry");
    assert_eq!(rotation_replay.status, OperationStatusV1::Replayed);
    assert_eq!(rotation_replay.result_json, rotated.result_json);
    assert_eq!(
        service.provenance().expect("rotation replay provenance"),
        after_rotation
    );
    assert_eq!(
        service.administer(
            AuthorityAdminCommandV1::Rotate {
                operation_id: [0x61; 32],
                expected_audit_head: before.audit_head,
                expected_key_revision: 1,
                new_key_revision: 2,
                new_policy_revision: 2,
                new_policy_digest: [0x65; 32],
            },
            TEST_NOW_MILLIS_V1 + 1,
        ),
        Err(TairaAuthorityErrorV1::Conflict)
    );
    assert_eq!(
        service.assign_run_json(&old_assignment, TEST_NOW_MILLIS_V1),
        Err(TairaAuthorityErrorV1::Conflict),
        "an assignment signed for the predecessor policy cannot cross rotation"
    );
    let historical = service
        .verify_json(
            &verification_json(&fixture, &old_authorization),
            read_only_descriptors(&[&artifact]),
            service
                .public_binding()
                .expect("historical verification binding")
                .signer
                .client_uid,
        )
        .expect("verify predecessor receipt after rotation");
    assert_eq!(historical.status, OperationStatusV1::Ok);
    assert_eq!(result_status(&historical), "valid");
    assert_eq!(
        service
            .provenance()
            .expect("historical verification provenance"),
        after_rotation
    );

    let revoke = AuthorityAdminCommandV1::Revoke {
        operation_id: [0x63; 32],
        expected_audit_head: after_rotation.audit_head,
        expected_key_revision: 2,
        reason_digest: [0x64; 32],
    };
    let revoked = service
        .administer(revoke.clone(), TEST_NOW_MILLIS_V1 + 1)
        .expect("revoke authority");
    assert_eq!(revoked.status, OperationStatusV1::Ok);
    assert_eq!(result_status(&revoked), "revoked");
    let after_revoke = service.provenance().expect("revoked provenance");
    assert!(after_revoke.revoked);
    assert_eq!(
        after_revoke.audit_sequence,
        after_rotation.audit_sequence + 1
    );

    let replayed_revoke = service
        .administer(revoke, TEST_NOW_MILLIS_V1 + 2)
        .expect("exact revoke retry");
    assert_eq!(replayed_revoke.status, OperationStatusV1::Ok);
    assert_eq!(
        service.provenance().expect("revoke replay provenance"),
        after_revoke
    );
    assert_eq!(
        service.assign_run_json(
            &fixture.assignment_json(
                &service,
                TEST_NOW_MILLIS_V1,
                TEST_NOW_MILLIS_V1,
                TEST_NOW_MILLIS_V1 + 100,
            ),
            TEST_NOW_MILLIS_V1 + 2,
        ),
        Err(TairaAuthorityErrorV1::Conflict)
    );

    drop(service);
    let recovered = TairaAuthorityServiceV1::open(state_directory(parent.path()), wrapping_key())
        .expect("open revoked authority");
    assert!(
        recovered
            .provenance()
            .expect("recovered revoked provenance")
            .revoked
    );
    assert_eq!(
        result_status(
            &recovered
                .administer(AuthorityAdminCommandV1::Status, TEST_NOW_MILLIS_V1 + 3)
                .expect("recovered status"),
        ),
        "revoked"
    );
}

#[test]
fn canonical_frames_reject_truncation_wrong_magic_and_oversize() {
    let request = AuthorizeRequestV1 {
        binding_sha256: [0x71; 32],
        request_json: br#"{"request":"canonical"}\n"#.to_vec(),
    };
    let encoded = encode_frame(FRAME_AUTHORIZE_REQUEST_V1, &request).expect("encode frame");
    let frame = decode_frame(&encoded).expect("decode frame");
    assert_eq!(frame.magic, TAIRA_AUTHORITY_PROTOCOL_MAGIC_V1);
    assert_eq!(frame.version, TAIRA_AUTHORITY_PROTOCOL_VERSION_V1);
    assert_eq!(frame.kind, FRAME_AUTHORIZE_REQUEST_V1);
    assert_eq!(
        decode_body::<AuthorizeRequestV1>(&frame.body).expect("decode request body"),
        request
    );

    for truncated in [&encoded[..0], &encoded[..1], &encoded[..encoded.len() - 1]] {
        assert!(decode_frame(truncated).is_err());
    }
    let wrong_magic = norito::encode_canonical(&AuthorityFrameV1 {
        magic: *b"IRTAUT00",
        version: TAIRA_AUTHORITY_PROTOCOL_VERSION_V1,
        kind: FRAME_AUTHORIZE_REQUEST_V1,
        body: norito::encode_canonical(&request).expect("encode request body"),
    })
    .expect("encode wrong-magic frame");
    assert!(decode_frame(&wrong_magic).is_err());

    let oversized_wire = vec![0_u8; TAIRA_AUTHORITY_MAX_FRAME_BYTES_V1 + 1];
    assert!(decode_frame(&oversized_wire).is_err());
    assert!(
        encode_frame(
            FRAME_AUTHORIZE_REQUEST_V1,
            &vec![0_u8; TAIRA_AUTHORITY_MAX_FRAME_BYTES_V1 + 1],
        )
        .is_err()
    );
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "deploy disposition lifecycle regression"
)]
fn deploy_dry_run_does_not_consume_and_apply_and_finalize_are_once_only() {
    let parent = temporary_parent();
    let artifact = create_artifact(parent.path(), "deployment.json", b"deployment");
    let fixture = ClientRequestFixtureV1::new(
        TairaAuthorityRoleV1::DeployIssuance,
        "deployment",
        &[("deployment.json", b"deployment")],
    );
    let service = provision(parent.path(), fixture.role);
    assign_active_run(&service, &fixture);
    let after_assignment = service.provenance().expect("assigned provenance");

    let dry_run_request = fixture.request_json_with_deploy(Some("dry-run"), None);
    let dry_run = authorize(
        &service,
        &dry_run_request,
        read_only_descriptors(&[&artifact]),
        TEST_NOW_MILLIS_V1,
    )
    .expect("dry-run authorization");
    assert_eq!(dry_run.status, OperationStatusV1::Ok);
    assert_eq!(result_status(&dry_run), "verified");
    assert_eq!(
        service.provenance().expect("dry-run provenance"),
        after_assignment,
        "dry-run must neither consume nor sign"
    );

    let finalization_result = [0x81; 32];
    let finalize_request =
        fixture.request_json_with_deploy(Some("finalize"), Some(("success", finalization_result)));
    assert_eq!(
        authorize(
            &service,
            &finalize_request,
            Vec::new(),
            TEST_NOW_MILLIS_V1 + 1,
        ),
        Err(TairaAuthorityErrorV1::Rejected),
        "a verified dry-run is not an applied authorization"
    );

    let apply_request = fixture.request_json_with_deploy(Some("apply"), None);
    let applied = authorize(
        &service,
        &apply_request,
        read_only_descriptors(&[&artifact]),
        TEST_NOW_MILLIS_V1 + 2,
    )
    .expect("apply authorization");
    assert_eq!(applied.status, OperationStatusV1::Ok);
    assert_eq!(result_status(&applied), "authorized");
    let after_apply = service.provenance().expect("apply provenance");
    assert_eq!(
        after_apply.audit_sequence,
        after_assignment.audit_sequence + 2
    );

    let apply_replay = authorize(
        &service,
        &apply_request,
        read_only_descriptors(&[&artifact]),
        TEST_NOW_MILLIS_V1 + 3,
    )
    .expect("apply retry");
    assert_eq!(apply_replay.status, OperationStatusV1::Replayed);
    assert_eq!(
        sidecar_bytes(&apply_replay, "authority_envelope"),
        sidecar_bytes(&applied, "authority_envelope")
    );
    assert_eq!(
        sidecar_bytes(&apply_replay, "durable_receipt"),
        sidecar_bytes(&applied, "durable_receipt")
    );
    assert_eq!(
        service.provenance().expect("apply replay provenance"),
        after_apply
    );

    let finalized = authorize(
        &service,
        &finalize_request,
        Vec::new(),
        TEST_NOW_MILLIS_V1 + 4,
    )
    .expect("finalize deployment");
    assert_eq!(finalized.status, OperationStatusV1::Ok);
    assert_eq!(result_status(&finalized), "finalized");
    let final_envelope = sidecar_bytes(&finalized, "authority_envelope");
    let final_receipt = sidecar_bytes(&finalized, "durable_receipt");
    let after_finalize = service.provenance().expect("finalized provenance");
    assert_eq!(
        after_finalize.audit_sequence,
        after_apply.audit_sequence + 2
    );

    let finalize_replay = authorize(
        &service,
        &finalize_request,
        Vec::new(),
        TEST_NOW_MILLIS_V1 + 5,
    )
    .expect("finalization retry");
    assert_eq!(finalize_replay.status, OperationStatusV1::Replayed);
    assert_eq!(result_status(&finalize_replay), "replayed");
    assert_eq!(
        sidecar_bytes(&finalize_replay, "authority_envelope"),
        final_envelope
    );
    assert_eq!(
        sidecar_bytes(&finalize_replay, "durable_receipt"),
        final_receipt
    );
    assert_eq!(
        service
            .provenance()
            .expect("finalization replay provenance"),
        after_finalize
    );

    let conflicting_finalize =
        fixture.request_json_with_deploy(Some("finalize"), Some(("rolled-back", [0x82; 32])));
    assert_eq!(
        authorize(
            &service,
            &conflicting_finalize,
            Vec::new(),
            TEST_NOW_MILLIS_V1 + 6,
        ),
        Err(TairaAuthorityErrorV1::Conflict)
    );

    drop(service);
    let recovered = TairaAuthorityServiceV1::open(state_directory(parent.path()), wrapping_key())
        .expect("recover finalized deployment authority");
    let recovered_replay = authorize(
        &recovered,
        &finalize_request,
        Vec::new(),
        TEST_NOW_MILLIS_V1 + 7,
    )
    .expect("replay finalization after recovery");
    assert_eq!(recovered_replay.status, OperationStatusV1::Replayed);
    assert_eq!(
        sidecar_bytes(&recovered_replay, "authority_envelope"),
        final_envelope
    );
    assert_eq!(
        sidecar_bytes(&recovered_replay, "durable_receipt"),
        final_receipt
    );
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "deployment finalization crash matrix"
)]
fn deployment_finalization_recovers_each_durable_crash_boundary() {
    for (case, phase, committed_at_crash) in [
        (
            "finalize-crash-after-input",
            DeploymentFinalizationCrashPhaseV1::AfterInputPersistence,
            0,
        ),
        (
            "finalize-crash-after-decision",
            DeploymentFinalizationCrashPhaseV1::AfterDecisionSignerCommit,
            1,
        ),
        (
            "finalize-crash-after-receipt",
            DeploymentFinalizationCrashPhaseV1::AfterDurableReceiptSignerCommit,
            2,
        ),
    ] {
        let parent = temporary_parent();
        let artifact = create_artifact(parent.path(), "deployment.json", b"deployment");
        let fixture = ClientRequestFixtureV1::new(
            TairaAuthorityRoleV1::DeployIssuance,
            case,
            &[("deployment.json", b"deployment")],
        );
        let service = provision(parent.path(), fixture.role);
        assign_active_run(&service, &fixture);

        let apply_request = fixture.request_json_with_deploy(Some("apply"), None);
        authorize(
            &service,
            &apply_request,
            read_only_descriptors(&[&artifact]),
            TEST_NOW_MILLIS_V1,
        )
        .expect("apply deployment before injected finalization crash");
        let after_apply = service.provenance().expect("applied provenance");
        let finalize_request =
            fixture.request_json_with_deploy(Some("finalize"), Some(("success", [0x91; 32])));
        service
            .inject_deployment_finalization_crash_for_test(phase)
            .expect("configure isolated finalization crash phase");
        assert_eq!(
            authorize(
                &service,
                &finalize_request,
                Vec::new(),
                TEST_NOW_MILLIS_V1 + 1,
            ),
            Err(TairaAuthorityErrorV1::State),
            "finalization did not stop at {phase:?}"
        );
        assert_eq!(
            service
                .provenance()
                .expect("crash-boundary provenance")
                .audit_sequence,
            after_apply.audit_sequence + committed_at_crash,
            "unexpected signer progress at {phase:?}"
        );

        let blocked_assignment = ClientRequestFixtureV1::new(
            TairaAuthorityRoleV1::DeployIssuance,
            &format!("{case}-blocked"),
            &[],
        );
        assert_eq!(
            service.assign_run_json(
                &blocked_assignment.assignment_json(
                    &service,
                    TEST_NOW_MILLIS_V1 - 1,
                    TEST_NOW_MILLIS_V1,
                    TEST_NOW_MILLIS_V1 + 60_000,
                ),
                TEST_NOW_MILLIS_V1,
            ),
            Err(TairaAuthorityErrorV1::Conflict),
            "assign-run appended across incomplete finalization at {phase:?}"
        );
        drop(service);

        let recovered =
            TairaAuthorityServiceV1::open(state_directory(parent.path()), wrapping_key())
                .expect("recover incomplete deployment finalization");
        let recovered_provenance = recovered
            .provenance()
            .expect("recovered finalization provenance");
        assert_eq!(
            recovered_provenance.audit_sequence,
            after_apply.audit_sequence + 2,
            "recovery did not complete both finalization commits at {phase:?}"
        );
        let replay = authorize(
            &recovered,
            &finalize_request,
            Vec::new(),
            TEST_NOW_MILLIS_V1 + 2,
        )
        .expect("replay recovered finalization");
        assert_eq!(replay.status, OperationStatusV1::Replayed);
        assert_eq!(result_status(&replay), "replayed");
        assert_eq!(
            recovered
                .provenance()
                .expect("replayed finalization provenance"),
            recovered_provenance,
            "replay appended after recovered finalization at {phase:?}"
        );
    }
}

include!("tests/privacy_governance_tests.rs");
