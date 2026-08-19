//! Focused authority protocol, durable-state, and service tests.

mod privacy_protocol_origin_service_tests;
mod public_soak_native_tests;
mod python_native_e2e;
mod qualification_service_tests;

use super::super::{
    SoftwareSignerKeyAlgorithmV1, SoftwareSignerPurposeBindingV1, SoftwareSignerRoleV1,
    SoftwareSignerWrappingKeyV1,
};
#[cfg(target_os = "linux")]
use super::transport::serve_one_kernel_peer_for_test;
use super::{
    TairaAuthorityClientV1, TairaAuthorityEndpointPolicyV1, TairaAuthorityInstallationV1,
    TairaAuthorityProvisioningV1, TairaAuthorityPublicBindingV1, TairaAuthorityRoleV1,
    TairaAuthorityServerV1, TairaAuthorityServiceV1,
    protocol::{
        AuthorityAdminCommandV1, AuthorityAdminRequestV1, AuthorityFrameV1, AuthorizeRequestV1,
        FRAME_ADMIN_REQUEST_V1, FRAME_ADMIN_RESPONSE_V1, FRAME_AUTHORIZE_REQUEST_V1,
        FRAME_QUALIFY_REQUEST_V1, FRAME_QUALIFY_RESPONSE_V1, OperationResponseV1,
        OperationStatusV1, QualifyRequestV1, QualifyResponseV1, ReplayConsumptionV1,
        StoredAuthorizationV1, StoredDeploymentFinalizationInputV1, StoredDeploymentFinalizationV1,
        StoredPublicSoakObservationBindingAnchorV1, StoredPublicSoakObservationBindingInputV1,
        TAIRA_AUTHORITY_MAX_FRAME_BYTES_V1, TAIRA_AUTHORITY_PROTOCOL_MAGIC_V1,
        TAIRA_AUTHORITY_PROTOCOL_VERSION_V1, decode_body, decode_frame, encode_frame,
        qualify_response_digest,
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
#[cfg(target_os = "linux")]
use std::os::unix::fs::{FileTypeExt as _, MetadataExt as _};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::{Read as _, Write as _},
    os::{
        fd::{AsFd as _, OwnedFd},
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
const TEST_GOVERNANCE_PUBLIC_KEY_V1: &str =
    "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03";
const TEST_GOVERNANCE_PRIVATE_KEY_V1: &str =
    "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53";

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

#[cfg(target_os = "linux")]
fn is_root_owned_nonwritable_executable_directory(path: &Path) -> bool {
    path.is_absolute()
        && path.ancestors().all(|ancestor| {
            fs::symlink_metadata(ancestor).is_ok_and(|metadata| {
                !metadata.file_type().is_symlink()
                    && metadata.is_dir()
                    && metadata.uid() == 0
                    && metadata.mode() & 0o022 == 0
                    && metadata.mode() & 0o111 == 0o111
            })
        })
}

fn eight_role_harness_parent() -> tempfile::TempDir {
    #[cfg(target_os = "linux")]
    if rustix::process::geteuid().as_raw() == 0 {
        for base in [Path::new("/var/lib"), Path::new("/run")] {
            if !is_root_owned_nonwritable_executable_directory(base) {
                continue;
            }
            let Ok(parent) = tempfile::Builder::new()
                .prefix("iroha-taira-authority-test-v1-")
                .tempdir_in(base)
            else {
                continue;
            };
            fs::set_permissions(parent.path(), fs::Permissions::from_mode(0o711))
                .expect("make privileged authority harness parent traversable");
            assert!(
                is_root_owned_nonwritable_executable_directory(parent.path()),
                "privileged authority harness parent must have only root-owned, non-writable executable ancestors",
            );
            return parent;
        }
        panic!(
            "privileged authority harness requires a writable root-owned 0755 /var/lib or /run base"
        );
    }
    temporary_parent()
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

fn verification_json_from_authorization_result(
    authorize_request_json: &[u8],
    authorization_result_json: &[u8],
) -> Vec<u8> {
    let mut request = parse_json(authorize_request_json)
        .as_object()
        .cloned()
        .expect("authorization request object");
    request.remove("disposition");
    request.remove("deployment_result");
    request.insert(
        "schema".into(),
        Value::from("iroha.taira.authority-client-verification.v1"),
    );
    let result = parse_json(authorization_result_json);
    request.insert(
        "authority_envelope".into(),
        result
            .get("authority_envelope")
            .cloned()
            .expect("authorization authority envelope"),
    );
    request.insert(
        "durable_receipt".into(),
        result
            .get("durable_receipt")
            .cloned()
            .expect("authorization durable receipt"),
    );
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

fn registry_governance_retained_key() -> KeyPair {
    KeyPair::new(
        TEST_GOVERNANCE_PUBLIC_KEY_V1
            .parse()
            .expect("retained governance public key"),
        TEST_GOVERNANCE_PRIVATE_KEY_V1
            .parse()
            .expect("retained governance private key"),
    )
    .expect("matching retained governance keypair")
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
                TairaAuthorityServiceV1::provision_with_retained_genesis_key_for_test(
                    &state,
                    provisioning,
                    registry_wrapping_key(ordinal),
                    registry_governance_retained_key(),
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
        let runtime = parent.join(format!("{ordinal:02}-{}-runtime", role.as_str()));
        installations.push(TairaAuthorityInstallationV1 {
            binding,
            state_directory: state,
            request_socket: runtime.join("request.sock"),
            administrator_socket: runtime.join("administrator.sock"),
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

const KERNEL_PEER_CHILD_SOCKET_ENV_V1: &str = "IROHA_TAIRA_TEST_KERNEL_PEER_SOCKET_V1";
const KERNEL_PEER_CHILD_BINDING_ENV_V1: &str = "IROHA_TAIRA_TEST_KERNEL_PEER_BINDING_SHA256_V1";
const KERNEL_PEER_CHILD_NONCE_ENV_V1: &str = "IROHA_TAIRA_TEST_KERNEL_PEER_NONCE_V1";
const KERNEL_PEER_CHILD_ROLE_ENV_V1: &str = "IROHA_TAIRA_TEST_KERNEL_PEER_ROLE_V1";
const KERNEL_PEER_CHILD_REJECTION_ENV_V1: &str = "IROHA_TAIRA_TEST_KERNEL_PEER_EXPECT_REJECTION_V1";
const KERNEL_PEER_CHILD_TEST_NAME_V1: &str =
    "external_software_signer::taira_authority::tests::kernel_peer_qualify_child_v1";
const KERNEL_PEER_SERVICE_STATE_ENV_V1: &str = "IROHA_TAIRA_TEST_SERVICE_STATE_V1";
const KERNEL_PEER_SERVICE_REQUEST_SOCKET_ENV_V1: &str =
    "IROHA_TAIRA_TEST_SERVICE_REQUEST_SOCKET_V1";
const KERNEL_PEER_SERVICE_ADMIN_SOCKET_ENV_V1: &str = "IROHA_TAIRA_TEST_SERVICE_ADMIN_SOCKET_V1";
const KERNEL_PEER_SERVICE_ORDINAL_ENV_V1: &str = "IROHA_TAIRA_TEST_SERVICE_ORDINAL_V1";
const KERNEL_PEER_SERVICE_NOW_ENV_V1: &str = "IROHA_TAIRA_TEST_SERVICE_NOW_V1";
#[cfg(target_os = "linux")]
const KERNEL_PEER_SERVICE_TEST_NAME_V1: &str =
    "external_software_signer::taira_authority::tests::kernel_peer_service_child_v1";
const KERNEL_PEER_ADMIN_ASSIGNMENT_ENV_V1: &str = "IROHA_TAIRA_TEST_ADMIN_ASSIGNMENT_V1";
#[cfg(target_os = "linux")]
const KERNEL_PEER_ADMIN_TEST_NAME_V1: &str =
    "external_software_signer::taira_authority::tests::kernel_peer_administrator_child_v1";
const KERNEL_PEER_CLIENT_BINDING_ENV_V1: &str = "IROHA_TAIRA_TEST_CLIENT_BINDING_V1";
const KERNEL_PEER_CLIENT_REQUEST_ENV_V1: &str = "IROHA_TAIRA_TEST_CLIENT_REQUEST_V1";
const KERNEL_PEER_CLIENT_ARTIFACT_DIRECTORY_ENV_V1: &str =
    "IROHA_TAIRA_TEST_CLIENT_ARTIFACT_DIRECTORY_V1";
const KERNEL_PEER_CLIENT_ARTIFACT_COUNT_ENV_V1: &str = "IROHA_TAIRA_TEST_CLIENT_ARTIFACT_COUNT_V1";
#[cfg(target_os = "linux")]
const KERNEL_PEER_CLIENT_TEST_NAME_V1: &str =
    "external_software_signer::taira_authority::tests::kernel_peer_client_child_v1";
const KERNEL_PEER_CLIENT_RESULT_MARKER_V1: &str = "TAIRA_KERNEL_CLIENT_RESULT_V1=";

#[cfg(target_os = "linux")]
fn privileged_kernel_peer_child_command(child_binary: &Path, uid: u32) -> std::process::Command {
    let setpriv = [Path::new("/usr/bin/setpriv"), Path::new("/bin/setpriv")]
        .into_iter()
        .find(|candidate| {
            fs::symlink_metadata(candidate).is_ok_and(|metadata| {
                !metadata.file_type().is_symlink()
                    && metadata.is_file()
                    && metadata.uid() == 0
                    && metadata.mode() & 0o022 == 0
                    && metadata.mode() & 0o111 != 0
                    && candidate
                        .parent()
                        .is_some_and(is_root_owned_nonwritable_executable_directory)
            })
        })
        .expect("privileged kernel-peer harness requires a trusted setpriv executable");
    let mut command = std::process::Command::new(setpriv);
    command
        .arg("--reuid")
        .arg(uid.to_string())
        .arg("--regid")
        .arg(uid.to_string())
        .arg("--clear-groups")
        .arg("--")
        .arg(child_binary)
        .env_clear();
    command
}

fn assert_sanitized_kernel_peer_child(expected_environment: &[&str]) {
    #[cfg(target_os = "linux")]
    {
        let observed_environment = std::env::vars_os()
            .map(|(name, _)| {
                name.into_string()
                    .expect("kernel-peer child environment name must be ASCII")
            })
            .collect::<BTreeSet<_>>();
        let expected_environment = expected_environment
            .iter()
            .map(|name| (*name).to_owned())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            observed_environment, expected_environment,
            "kernel-peer child inherited an unexpected environment variable",
        );
        let process_status =
            fs::read_to_string("/proc/self/status").expect("read kernel-peer child process status");
        let supplementary_groups = process_status
            .lines()
            .find_map(|line| line.strip_prefix("Groups:"))
            .expect("kernel-peer child process status groups");
        assert!(
            supplementary_groups.trim().is_empty(),
            "kernel-peer child retained supplementary groups",
        );
    }
    #[cfg(not(target_os = "linux"))]
    let _ = expected_environment;
}

#[test]
fn kernel_peer_qualify_child_v1() {
    let Some(socket) = std::env::var_os(KERNEL_PEER_CHILD_SOCKET_ENV_V1) else {
        return;
    };
    let expect_rejection = std::env::var_os(KERNEL_PEER_CHILD_REJECTION_ENV_V1).is_some();
    let mut expected_environment = vec![
        KERNEL_PEER_CHILD_SOCKET_ENV_V1,
        KERNEL_PEER_CHILD_BINDING_ENV_V1,
        KERNEL_PEER_CHILD_NONCE_ENV_V1,
        KERNEL_PEER_CHILD_ROLE_ENV_V1,
    ];
    if expect_rejection {
        expected_environment.push(KERNEL_PEER_CHILD_REJECTION_ENV_V1);
    }
    assert_sanitized_kernel_peer_child(&expected_environment);
    let mut stream = UnixStream::connect(PathBuf::from(socket)).expect("connect kernel-peer child");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("set kernel-peer child read timeout");
    if expect_rejection {
        let mut byte = [0_u8; 1];
        match stream.read(&mut byte) {
            Ok(0) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::BrokenPipe
                ) => {}
            result => panic!("wrong kernel peer was not rejected before frame input: {result:?}"),
        }
        return;
    }

    let binding_hex =
        std::env::var(KERNEL_PEER_CHILD_BINDING_ENV_V1).expect("kernel-peer binding digest");
    let mut binding_sha256 = [0_u8; 32];
    hex::decode_to_slice(binding_hex, &mut binding_sha256)
        .expect("decode kernel-peer binding digest");
    let nonce_byte = std::env::var(KERNEL_PEER_CHILD_NONCE_ENV_V1)
        .expect("kernel-peer nonce")
        .parse::<u8>()
        .expect("parse kernel-peer nonce");
    let nonce = [nonce_byte; 32];
    let encoded = encode_frame(
        FRAME_QUALIFY_REQUEST_V1,
        &QualifyRequestV1 {
            binding_sha256,
            client_nonce: nonce,
        },
    )
    .expect("encode canonical kernel-peer qualify request");
    stream
        .write_all(
            &u32::try_from(encoded.len())
                .expect("kernel-peer frame length")
                .to_be_bytes(),
        )
        .and_then(|()| stream.write_all(&encoded))
        .and_then(|()| stream.flush())
        .expect("send canonical kernel-peer qualify request");

    let mut prefix = [0_u8; 4];
    stream
        .read_exact(&mut prefix)
        .expect("read kernel-peer response prefix");
    let response_length =
        usize::try_from(u32::from_be_bytes(prefix)).expect("kernel-peer response length");
    assert!(response_length <= TAIRA_AUTHORITY_MAX_FRAME_BYTES_V1);
    let mut response = vec![0_u8; response_length];
    stream
        .read_exact(&mut response)
        .expect("read kernel-peer response");
    let frame = decode_frame(&response).expect("decode canonical kernel-peer response");
    assert_eq!(frame.magic, TAIRA_AUTHORITY_PROTOCOL_MAGIC_V1);
    assert_eq!(frame.version, TAIRA_AUTHORITY_PROTOCOL_VERSION_V1);
    assert_eq!(frame.kind, FRAME_QUALIFY_RESPONSE_V1);
    let qualified: QualifyResponseV1 =
        decode_body(&frame.body).expect("decode kernel-peer qualify response");
    assert_eq!(qualified.client_nonce, nonce);
    assert_ne!(qualified.server_nonce, [0; 32]);
    assert_eq!(
        qualified.response_digest,
        qualify_response_digest(&qualified).expect("kernel-peer response digest")
    );
    assert!(!qualified.response_attestation.is_empty());
    let status = parse_json(&qualified.status_json);
    assert_eq!(
        status["role"],
        Value::from(std::env::var(KERNEL_PEER_CHILD_ROLE_ENV_V1).expect("kernel-peer role"))
    );
    assert_eq!(status["status"], Value::from("ready"));
}

#[test]
fn kernel_peer_service_child_v1() {
    let Some(state_directory) = std::env::var_os(KERNEL_PEER_SERVICE_STATE_ENV_V1) else {
        return;
    };
    assert_sanitized_kernel_peer_child(&[
        KERNEL_PEER_SERVICE_STATE_ENV_V1,
        KERNEL_PEER_SERVICE_REQUEST_SOCKET_ENV_V1,
        KERNEL_PEER_SERVICE_ADMIN_SOCKET_ENV_V1,
        KERNEL_PEER_SERVICE_ORDINAL_ENV_V1,
        KERNEL_PEER_SERVICE_NOW_ENV_V1,
    ]);
    let ordinal = std::env::var(KERNEL_PEER_SERVICE_ORDINAL_ENV_V1)
        .expect("kernel-peer service ordinal")
        .parse::<usize>()
        .expect("parse kernel-peer service ordinal");
    let now_unix_millis = std::env::var(KERNEL_PEER_SERVICE_NOW_ENV_V1)
        .expect("kernel-peer service time")
        .parse::<u64>()
        .expect("parse kernel-peer service time");
    let service = Arc::new(
        TairaAuthorityServiceV1::open(
            PathBuf::from(state_directory),
            registry_wrapping_key(ordinal),
        )
        .expect("production-open transferred role state"),
    );
    let binding = service
        .public_binding()
        .expect("kernel-peer service binding");
    assert_eq!(binding.role, TairaAuthorityRoleV1::ALL[ordinal]);
    let policy = TairaAuthorityEndpointPolicyV1::try_new(
        PathBuf::from(
            std::env::var_os(KERNEL_PEER_SERVICE_REQUEST_SOCKET_ENV_V1)
                .expect("kernel-peer service request socket"),
        ),
        PathBuf::from(
            std::env::var_os(KERNEL_PEER_SERVICE_ADMIN_SOCKET_ENV_V1)
                .expect("kernel-peer service administrator socket"),
        ),
        binding,
    )
    .expect("kernel-peer service endpoint policy");
    TairaAuthorityServerV1::try_new(service, policy)
        .expect("bind transferred role service identity")
        .serve_assignment_and_request_sessions_for_test(
            now_unix_millis.saturating_sub(5),
            &[now_unix_millis; 7],
        )
        .expect("serve one administrator assignment and finite actual-client session sequence");
}

#[test]
fn kernel_peer_administrator_child_v1() {
    let Some(encoded_binding) = std::env::var_os(KERNEL_PEER_CLIENT_BINDING_ENV_V1) else {
        return;
    };
    assert_sanitized_kernel_peer_child(&[
        KERNEL_PEER_SERVICE_REQUEST_SOCKET_ENV_V1,
        KERNEL_PEER_SERVICE_ADMIN_SOCKET_ENV_V1,
        KERNEL_PEER_CLIENT_BINDING_ENV_V1,
        KERNEL_PEER_ADMIN_ASSIGNMENT_ENV_V1,
    ]);
    let binding_bytes = hex::decode(
        encoded_binding
            .into_string()
            .expect("ASCII kernel-peer administrator binding"),
    )
    .expect("decode kernel-peer administrator binding");
    let binding: TairaAuthorityPublicBindingV1 =
        norito::decode_canonical(&binding_bytes).expect("decode canonical administrator binding");
    let role = binding.role;
    let client = TairaAuthorityClientV1::new(
        TairaAuthorityEndpointPolicyV1::try_new(
            PathBuf::from(
                std::env::var_os(KERNEL_PEER_SERVICE_REQUEST_SOCKET_ENV_V1)
                    .expect("kernel-peer administrator request socket"),
            ),
            PathBuf::from(
                std::env::var_os(KERNEL_PEER_SERVICE_ADMIN_SOCKET_ENV_V1)
                    .expect("kernel-peer administrator socket"),
            ),
            binding,
        )
        .expect("kernel-peer administrator endpoint policy"),
    );
    let assignment_json = fs::read(
        std::env::var_os(KERNEL_PEER_ADMIN_ASSIGNMENT_ENV_V1)
            .expect("kernel-peer administrator assignment path"),
    )
    .expect("read kernel-peer administrator assignment");
    let assigned = client
        .assign_run_for_test(assignment_json)
        .expect("actual administrator client run assignment");
    let result = parse_json(&assigned);
    assert_eq!(
        result.get("schema").and_then(Value::as_str),
        Some("iroha.taira.authority-run-assignment-result.v1"),
    );
    assert_eq!(
        result.get("role").and_then(Value::as_str),
        Some(role.as_str()),
    );
    assert_eq!(
        result.get("status").and_then(Value::as_str),
        Some("assigned"),
    );
}

fn kernel_status_audit_position(status_json: &[u8]) -> (u64, String) {
    let status = parse_json(status_json);
    (
        status
            .get("audit_sequence")
            .and_then(Value::as_u64)
            .expect("kernel-peer status audit sequence"),
        status
            .get("audit_head")
            .and_then(Value::as_str)
            .expect("kernel-peer status audit head")
            .to_owned(),
    )
}

#[test]
fn kernel_peer_client_child_v1() {
    let Some(encoded_binding) = std::env::var_os(KERNEL_PEER_CLIENT_BINDING_ENV_V1) else {
        return;
    };
    assert_sanitized_kernel_peer_child(&[
        KERNEL_PEER_SERVICE_REQUEST_SOCKET_ENV_V1,
        KERNEL_PEER_SERVICE_ADMIN_SOCKET_ENV_V1,
        KERNEL_PEER_CLIENT_BINDING_ENV_V1,
        KERNEL_PEER_CLIENT_REQUEST_ENV_V1,
        KERNEL_PEER_CLIENT_ARTIFACT_DIRECTORY_ENV_V1,
        KERNEL_PEER_CLIENT_ARTIFACT_COUNT_ENV_V1,
    ]);
    let binding_bytes = hex::decode(
        encoded_binding
            .into_string()
            .expect("ASCII kernel-peer client binding"),
    )
    .expect("decode kernel-peer client binding");
    let binding: TairaAuthorityPublicBindingV1 =
        norito::decode_canonical(&binding_bytes).expect("decode canonical client binding");
    let client = TairaAuthorityClientV1::new(
        TairaAuthorityEndpointPolicyV1::try_new(
            PathBuf::from(
                std::env::var_os(KERNEL_PEER_SERVICE_REQUEST_SOCKET_ENV_V1)
                    .expect("kernel-peer client request socket"),
            ),
            PathBuf::from(
                std::env::var_os(KERNEL_PEER_SERVICE_ADMIN_SOCKET_ENV_V1)
                    .expect("kernel-peer client administrator socket"),
            ),
            binding,
        )
        .expect("kernel-peer client endpoint policy"),
    );

    // Authenticate the installed binding, socket owner, connected service
    // peer, signed status, and live availability before caller-controlled
    // request or artifact paths are read.
    client
        .status()
        .expect("initial authenticated client status");
    let authorize_request = fs::read(
        std::env::var_os(KERNEL_PEER_CLIENT_REQUEST_ENV_V1)
            .expect("kernel-peer client request path"),
    )
    .expect("read kernel-peer authorization request");
    let artifact_directory = PathBuf::from(
        std::env::var_os(KERNEL_PEER_CLIENT_ARTIFACT_DIRECTORY_ENV_V1)
            .expect("kernel-peer client artifact directory"),
    );
    let artifact_count = std::env::var(KERNEL_PEER_CLIENT_ARTIFACT_COUNT_ENV_V1)
        .expect("kernel-peer client artifact count")
        .parse::<usize>()
        .expect("parse kernel-peer client artifact count");
    let artifacts = (0..artifact_count)
        .map(|ordinal| {
            OpenOptions::new()
                .read(true)
                .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
                .open(artifact_directory.join(format!("{ordinal:02}.artifact")))
                .expect("open immutable kernel-peer artifact")
        })
        .collect::<Vec<_>>();
    let descriptors = artifacts
        .iter()
        .map(|artifact| artifact.as_fd())
        .collect::<Vec<_>>();

    let fresh = client
        .authorize(authorize_request.clone(), &descriptors)
        .expect("actual client fresh authorization");
    assert_eq!(
        parse_json(&fresh).get("status").and_then(Value::as_str),
        Some("authorized")
    );
    let after_fresh = client.status().expect("status after fresh authorization");
    let retry = client
        .authorize(authorize_request.clone(), &descriptors)
        .expect("actual client exact retry");
    assert_eq!(
        parse_json(&retry).get("status").and_then(Value::as_str),
        Some("replayed")
    );
    let fresh_value = parse_json(&fresh);
    let retry_value = parse_json(&retry);
    for sidecar in ["authority_envelope", "durable_receipt"] {
        let fresh_sidecar_bytes = canonical_json_core(
            fresh_value
                .get(sidecar)
                .unwrap_or_else(|| panic!("fresh result omitted {sidecar}")),
        );
        let retry_sidecar_bytes = canonical_json_core(
            retry_value
                .get(sidecar)
                .unwrap_or_else(|| panic!("retry result omitted {sidecar}")),
        );
        assert_eq!(
            fresh_sidecar_bytes, retry_sidecar_bytes,
            "exact retry changed canonical {sidecar} sidecar bytes",
        );
    }
    let after_retry = client.status().expect("status after exact retry");
    assert_eq!(
        kernel_status_audit_position(&after_retry),
        kernel_status_audit_position(&after_fresh),
        "exact retry appended authority provenance",
    );

    let verification = verification_json_from_authorization_result(&authorize_request, &fresh);
    let verified = client
        .verify_receipt(verification, &descriptors)
        .expect("actual client historical verification");
    assert_eq!(
        parse_json(&verified).get("status").and_then(Value::as_str),
        Some("valid")
    );
    let after_verify = client
        .status()
        .expect("status after historical verification");
    assert_eq!(
        kernel_status_audit_position(&after_verify),
        kernel_status_audit_position(&after_fresh),
        "historical verification appended authority provenance",
    );
    println!(
        "{KERNEL_PEER_CLIENT_RESULT_MARKER_V1}{}",
        hex::encode(fresh)
    );
}

#[cfg(target_os = "linux")]
fn kernel_artifact_manifest_value(artifacts: &[(String, Vec<u8>)]) -> Value {
    Value::Array(
        artifacts
            .iter()
            .enumerate()
            .map(|(ordinal, (name, bytes))| {
                let mut entry = Map::new();
                entry.insert("name".into(), Value::from(name.clone()));
                entry.insert("ordinal".into(), Value::from(ordinal as u64));
                entry.insert(
                    "sha256".into(),
                    Value::from(hex::encode(Sha256::digest(bytes))),
                );
                entry.insert("size".into(), Value::from(bytes.len() as u64));
                Value::Object(entry)
            })
            .collect(),
    )
}

#[cfg(target_os = "linux")]
fn kernel_peer_assignment_json(
    service: &TairaAuthorityServiceV1,
    fixture: &ClientRequestFixtureV1,
    now_unix_millis: u64,
) -> Vec<u8> {
    fixture.assignment_json(
        service,
        now_unix_millis.saturating_sub(20),
        now_unix_millis.saturating_sub(10),
        now_unix_millis
            .checked_add(90_000)
            .expect("kernel-peer assignment expiry"),
    )
}

#[cfg(target_os = "linux")]
fn transfer_kernel_role_tree(path: &Path, owner_uid: u32) {
    let metadata = fs::symlink_metadata(path).expect("inspect kernel role state entry");
    assert!(!metadata.file_type().is_symlink());
    if metadata.is_dir() {
        for entry in fs::read_dir(path).expect("read kernel role state directory") {
            transfer_kernel_role_tree(&entry.expect("kernel role state entry").path(), owner_uid);
        }
    } else {
        assert!(metadata.is_file());
        assert_eq!(metadata.nlink(), 1);
    }
    rustix::fs::chown(
        path,
        Some(rustix::process::Uid::from_raw(owner_uid)),
        Some(rustix::process::Gid::from_raw(owner_uid)),
    )
    .expect("transfer kernel role state ownership");
}

#[cfg(target_os = "linux")]
fn prepare_kernel_role_ownership(installation: &TairaAuthorityInstallationV1) {
    let runtime = installation
        .request_socket
        .parent()
        .expect("kernel role runtime directory");
    assert_eq!(
        Some(runtime),
        installation.administrator_socket.parent(),
        "role sockets must share one isolated runtime directory",
    );
    let mut builder = fs::DirBuilder::new();
    builder.mode(0o711);
    builder
        .create(runtime)
        .expect("create isolated kernel role runtime directory");
    rustix::fs::chown(
        runtime,
        Some(rustix::process::Uid::from_raw(
            installation.binding.signer.service_uid,
        )),
        Some(rustix::process::Gid::from_raw(
            installation.binding.signer.service_uid,
        )),
    )
    .expect("transfer kernel role runtime ownership");
    fs::set_permissions(runtime, fs::Permissions::from_mode(0o711))
        .expect("fix kernel role runtime permissions");
    transfer_kernel_role_tree(
        &installation.state_directory,
        installation.binding.signer.service_uid,
    );
}

#[cfg(target_os = "linux")]
fn stage_kernel_client_input(
    parent: &Path,
    ordinal: usize,
    assignment_json: &[u8],
    request_json: &[u8],
    artifact_payloads: &[Vec<u8>],
) -> (PathBuf, PathBuf, PathBuf) {
    let directory = parent.join(format!("{ordinal:02}-client-input"));
    let mut builder = fs::DirBuilder::new();
    builder.mode(0o711);
    builder
        .create(&directory)
        .expect("create kernel client input directory");
    let assignment_path = directory.join("assignment.json");
    let mut assignment = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&assignment_path)
        .expect("create kernel administrator assignment");
    assignment
        .write_all(assignment_json)
        .and_then(|()| assignment.sync_all())
        .expect("persist kernel administrator assignment");
    drop(assignment);
    fs::set_permissions(&assignment_path, fs::Permissions::from_mode(0o444))
        .expect("make kernel administrator assignment immutable");
    let request_path = directory.join("request.json");
    let mut request = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&request_path)
        .expect("create kernel client request");
    request
        .write_all(request_json)
        .and_then(|()| request.sync_all())
        .expect("persist kernel client request");
    drop(request);
    fs::set_permissions(&request_path, fs::Permissions::from_mode(0o444))
        .expect("make kernel client request immutable");
    for (artifact_ordinal, payload) in artifact_payloads.iter().enumerate() {
        let path = directory.join(format!("{artifact_ordinal:02}.artifact"));
        let mut artifact = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .expect("create kernel client artifact");
        artifact
            .write_all(payload)
            .and_then(|()| artifact.sync_all())
            .expect("persist kernel client artifact");
        drop(artifact);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o444))
            .expect("make kernel client artifact immutable");
    }
    (assignment_path, request_path, directory)
}

#[cfg(target_os = "linux")]
#[allow(clippy::too_many_arguments)]
fn run_actual_kernel_client_role(
    ordinal: usize,
    service: &TairaAuthorityServiceV1,
    installation: &TairaAuthorityInstallationV1,
    fixture: &ClientRequestFixtureV1,
    authorize_request: &[u8],
    artifact_payloads: &[Vec<u8>],
    now_unix_millis: u64,
    transport_parent: &Path,
    child_binary: &Path,
) -> Vec<u8> {
    use std::{process::Stdio, thread, time::Instant};

    assert_eq!(fixture.role, installation.binding.role);
    let assignment_json = kernel_peer_assignment_json(service, fixture, now_unix_millis);
    let (assignment_path, request_path, artifact_directory) = stage_kernel_client_input(
        transport_parent,
        ordinal,
        &assignment_json,
        authorize_request,
        artifact_payloads,
    );
    prepare_kernel_role_ownership(installation);

    let mut server =
        privileged_kernel_peer_child_command(child_binary, installation.binding.signer.service_uid);
    server
        .arg("--exact")
        .arg(KERNEL_PEER_SERVICE_TEST_NAME_V1)
        .arg("--nocapture")
        .current_dir("/")
        .env(
            KERNEL_PEER_SERVICE_STATE_ENV_V1,
            &installation.state_directory,
        )
        .env(
            KERNEL_PEER_SERVICE_REQUEST_SOCKET_ENV_V1,
            &installation.request_socket,
        )
        .env(
            KERNEL_PEER_SERVICE_ADMIN_SOCKET_ENV_V1,
            &installation.administrator_socket,
        )
        .env(KERNEL_PEER_SERVICE_ORDINAL_ENV_V1, ordinal.to_string())
        .env(KERNEL_PEER_SERVICE_NOW_ENV_V1, now_unix_millis.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut server = server.spawn().expect("spawn role service child");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match (
            fs::symlink_metadata(&installation.request_socket),
            fs::symlink_metadata(&installation.administrator_socket),
        ) {
            (Ok(request), Ok(administrator))
                if [request, administrator].iter().all(|metadata| {
                    metadata.file_type().is_socket()
                        && metadata.uid() == installation.binding.signer.service_uid
                        && metadata.mode() & 0o7777 == 0o666
                }) =>
            {
                break;
            }
            (Ok(_), Ok(_)) => panic!("role service created an invalid authority socket"),
            (request, administrator)
                if [request.as_ref().err(), administrator.as_ref().err()]
                    .into_iter()
                    .flatten()
                    .all(|error| error.kind() == std::io::ErrorKind::NotFound) =>
            {
                if let Some(status) = server.try_wait().expect("poll role service child") {
                    panic!("role service child exited before binding: {status}");
                }
                if Instant::now() >= deadline {
                    let _ = server.kill();
                    let _ = server.wait();
                    panic!("role service child did not bind before timeout");
                }
                thread::sleep(Duration::from_millis(5));
            }
            (request, administrator) => panic!(
                "inspect role authority sockets: request={request:?} administrator={administrator:?}"
            ),
        }
    }

    let encoded_binding =
        norito::encode_canonical(&installation.binding).expect("encode kernel client binding");
    let mut administrator = privileged_kernel_peer_child_command(
        child_binary,
        installation.binding.signer.administrator_uid,
    );
    administrator
        .arg("--exact")
        .arg(KERNEL_PEER_ADMIN_TEST_NAME_V1)
        .arg("--nocapture")
        .current_dir("/")
        .env(
            KERNEL_PEER_SERVICE_REQUEST_SOCKET_ENV_V1,
            &installation.request_socket,
        )
        .env(
            KERNEL_PEER_SERVICE_ADMIN_SOCKET_ENV_V1,
            &installation.administrator_socket,
        )
        .env(
            KERNEL_PEER_CLIENT_BINDING_ENV_V1,
            hex::encode(&encoded_binding),
        )
        .env(KERNEL_PEER_ADMIN_ASSIGNMENT_ENV_V1, &assignment_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let administrator_output = administrator
        .spawn()
        .expect("spawn role administrator child")
        .wait_with_output()
        .expect("wait for role administrator child");
    if !administrator_output.status.success() {
        let _ = server.kill();
        let server_output = server
            .wait_with_output()
            .expect("wait for failed role service child");
        panic!(
            "role administrator child failed: administrator stdout={} administrator stderr={} server stdout={} server stderr={}",
            String::from_utf8_lossy(&administrator_output.stdout),
            String::from_utf8_lossy(&administrator_output.stderr),
            String::from_utf8_lossy(&server_output.stdout),
            String::from_utf8_lossy(&server_output.stderr),
        );
    }

    let mut client =
        privileged_kernel_peer_child_command(child_binary, installation.binding.signer.client_uid);
    client
        .arg("--exact")
        .arg(KERNEL_PEER_CLIENT_TEST_NAME_V1)
        .arg("--nocapture")
        .current_dir("/")
        .env(
            KERNEL_PEER_SERVICE_REQUEST_SOCKET_ENV_V1,
            &installation.request_socket,
        )
        .env(
            KERNEL_PEER_SERVICE_ADMIN_SOCKET_ENV_V1,
            &installation.administrator_socket,
        )
        .env(
            KERNEL_PEER_CLIENT_BINDING_ENV_V1,
            hex::encode(encoded_binding),
        )
        .env(KERNEL_PEER_CLIENT_REQUEST_ENV_V1, &request_path)
        .env(
            KERNEL_PEER_CLIENT_ARTIFACT_DIRECTORY_ENV_V1,
            &artifact_directory,
        )
        .env(
            KERNEL_PEER_CLIENT_ARTIFACT_COUNT_ENV_V1,
            artifact_payloads.len().to_string(),
        )
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let client_output = client
        .spawn()
        .expect("spawn role client child")
        .wait_with_output()
        .expect("wait for role client child");
    if !client_output.status.success() {
        let _ = server.kill();
        let server_output = server
            .wait_with_output()
            .expect("wait for failed role service child");
        panic!(
            "role client child failed: client stdout={} client stderr={} server stdout={} server stderr={}",
            String::from_utf8_lossy(&client_output.stdout),
            String::from_utf8_lossy(&client_output.stderr),
            String::from_utf8_lossy(&server_output.stdout),
            String::from_utf8_lossy(&server_output.stderr),
        );
    }
    let server_output = server
        .wait_with_output()
        .expect("wait for role service child");
    assert!(
        server_output.status.success(),
        "role service child failed: stdout={} stderr={}",
        String::from_utf8_lossy(&server_output.stdout),
        String::from_utf8_lossy(&server_output.stderr),
    );
    let stdout = String::from_utf8(client_output.stdout).expect("UTF-8 role client output");
    let encoded = stdout
        .lines()
        .find_map(|line| {
            line.find(KERNEL_PEER_CLIENT_RESULT_MARKER_V1)
                .map(|index| &line[index + KERNEL_PEER_CLIENT_RESULT_MARKER_V1.len()..])
        })
        .expect("role client authorization result marker");
    hex::decode(encoded).expect("decode role client authorization result")
}

#[cfg(target_os = "linux")]
fn exercise_actual_kernel_client_roles(
    services: &[Arc<TairaAuthorityServiceV1>],
    installations: &[TairaAuthorityInstallationV1],
    transport_parent: &Path,
    child_binary: &Path,
) {
    let harness_parent = installations[0]
        .state_directory
        .parent()
        .expect("eight-role harness parent");
    assert!(
        installations
            .iter()
            .all(|installation| installation.state_directory.parent() == Some(harness_parent))
    );
    fs::set_permissions(harness_parent, fs::Permissions::from_mode(0o711))
        .expect("make eight-role harness parent traversable");

    #[cfg(all(
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    {
        let native_ordinal = TairaAuthorityRoleV1::ALL
            .iter()
            .position(|role| *role == TairaAuthorityRoleV1::NativeEvidence)
            .expect("native role ordinal");
        let native = super::native_evidence::tests::authority_service_fixture();
        let native_fixture = ClientRequestFixtureV1::from_subject_and_manifest(
            TairaAuthorityRoleV1::NativeEvidence,
            native.subject.clone(),
            native.manifest.clone(),
        );
        let native_payloads = native
            .paths
            .iter()
            .map(|path| fs::read(path).expect("read native kernel-client artifact"))
            .collect::<Vec<_>>();
        run_actual_kernel_client_role(
            native_ordinal,
            &services[native_ordinal],
            &installations[native_ordinal],
            &native_fixture,
            &native_fixture.request_json(),
            &native_payloads,
            TEST_NOW_MILLIS_V1,
            transport_parent,
            child_binary,
        );

        const GOVERNANCE_NOW_MILLIS_V1: u64 = 1_800_000_000_001;
        const GOVERNANCE_REQUEST_V1: &[u8] = include_bytes!(
            "../../../../../scripts/tests/fixtures/taira_privacy_governance_request_v1.json"
        );
        let governance_ordinal = TairaAuthorityRoleV1::ALL
            .iter()
            .position(|role| *role == TairaAuthorityRoleV1::PrivacyGovernance)
            .expect("governance role ordinal");
        let governance_fixture = ClientRequestFixtureV1::from_subject_and_manifest(
            TairaAuthorityRoleV1::PrivacyGovernance,
            parse_json(GOVERNANCE_REQUEST_V1),
            Value::Array(Vec::new()),
        );
        run_actual_kernel_client_role(
            governance_ordinal,
            &services[governance_ordinal],
            &installations[governance_ordinal],
            &governance_fixture,
            &governance_fixture.request_json(),
            &[],
            GOVERNANCE_NOW_MILLIS_V1,
            transport_parent,
            child_binary,
        );

        let qualification_ordinal = TairaAuthorityRoleV1::ALL
            .iter()
            .position(|role| *role == TairaAuthorityRoleV1::Qualification)
            .expect("qualification role ordinal");
        let qualification_artifacts = qualification_service_tests::qualification_artifacts();
        let qualification_refs = qualification_artifacts
            .iter()
            .map(|(name, payload)| (name.as_str(), payload.as_slice()))
            .collect::<Vec<_>>();
        let qualification_fixture = ClientRequestFixtureV1::new(
            TairaAuthorityRoleV1::Qualification,
            "actual-kernel-client",
            &qualification_refs,
        );
        let qualification_payloads = qualification_artifacts
            .into_iter()
            .map(|(_, payload)| payload)
            .collect::<Vec<_>>();
        run_actual_kernel_client_role(
            qualification_ordinal,
            &services[qualification_ordinal],
            &installations[qualification_ordinal],
            &qualification_fixture,
            &qualification_fixture.request_json(),
            &qualification_payloads,
            TEST_NOW_MILLIS_V1,
            transport_parent,
            child_binary,
        );

        let deploy_ordinal = TairaAuthorityRoleV1::ALL
            .iter()
            .position(|role| *role == TairaAuthorityRoleV1::DeployIssuance)
            .expect("deploy role ordinal");
        let deploy_payloads = vec![b"deployment".to_vec()];
        let deploy_refs = [("deployment.json", deploy_payloads[0].as_slice())];
        let deploy_fixture = ClientRequestFixtureV1::new(
            TairaAuthorityRoleV1::DeployIssuance,
            "actual-kernel-client",
            &deploy_refs,
        );
        let deploy_request = deploy_fixture.request_json_with_deploy(Some("apply"), None);
        run_actual_kernel_client_role(
            deploy_ordinal,
            &services[deploy_ordinal],
            &installations[deploy_ordinal],
            &deploy_fixture,
            &deploy_request,
            &deploy_payloads,
            TEST_NOW_MILLIS_V1,
            transport_parent,
            child_binary,
        );

        let rollout_ordinal = TairaAuthorityRoleV1::ALL
            .iter()
            .position(|role| *role == TairaAuthorityRoleV1::RolloutObservation)
            .expect("rollout role ordinal");
        let rollout_fixture = ClientRequestFixtureV1::new(
            TairaAuthorityRoleV1::RolloutObservation,
            "actual-kernel-client",
            &[],
        );
        run_actual_kernel_client_role(
            rollout_ordinal,
            &services[rollout_ordinal],
            &installations[rollout_ordinal],
            &rollout_fixture,
            &rollout_fixture.request_json(),
            &[],
            TEST_NOW_MILLIS_V1,
            transport_parent,
            child_binary,
        );
    }

    let origin_ordinal = TairaAuthorityRoleV1::ALL
        .iter()
        .position(|role| *role == TairaAuthorityRoleV1::PrivacyProtocolOrigin)
        .expect("origin role ordinal");
    let (origin_subject, origin_artifacts) =
        super::privacy_protocol_origin::tests::service_fixture_material();
    let origin_fixture = ClientRequestFixtureV1::from_subject_and_manifest(
        TairaAuthorityRoleV1::PrivacyProtocolOrigin,
        origin_subject,
        kernel_artifact_manifest_value(&origin_artifacts),
    );
    let origin_payloads = origin_artifacts
        .into_iter()
        .map(|(_, payload)| payload)
        .collect::<Vec<_>>();
    run_actual_kernel_client_role(
        origin_ordinal,
        &services[origin_ordinal],
        &installations[origin_ordinal],
        &origin_fixture,
        &origin_fixture.request_json(),
        &origin_payloads,
        TEST_NOW_MILLIS_V1,
        transport_parent,
        child_binary,
    );

    let observation_ordinal = TairaAuthorityRoleV1::ALL
        .iter()
        .position(|role| *role == TairaAuthorityRoleV1::PublicSoakObservation)
        .expect("observation role ordinal");
    let replay_ordinal = TairaAuthorityRoleV1::ALL
        .iter()
        .position(|role| *role == TairaAuthorityRoleV1::PublicSoakReplayAdmission)
        .expect("replay role ordinal");
    assert_ne!(
        installations[observation_ordinal].binding.signer.client_uid,
        installations[replay_ordinal].binding.signer.client_uid,
    );
    assert_ne!(
        installations[observation_ordinal]
            .binding
            .signer
            .service_uid,
        installations[replay_ordinal].binding.signer.service_uid,
    );
    assert_ne!(
        installations[observation_ordinal]
            .binding
            .signer
            .public_key_digest,
        installations[replay_ordinal]
            .binding
            .signer
            .public_key_digest,
    );
    let soak_subject = public_soak_native_tests::valid_public_soak_subject_core();
    let completed_at_unix_millis = TEST_NOW_MILLIS_V1 - 1_000;
    let observation_fixture = ClientRequestFixtureV1::from_subject_and_manifest(
        TairaAuthorityRoleV1::PublicSoakObservation,
        public_soak_native_tests::observation_subject(&soak_subject, completed_at_unix_millis),
        Value::Array(Vec::new()),
    );
    let observation_result = run_actual_kernel_client_role(
        observation_ordinal,
        &services[observation_ordinal],
        &installations[observation_ordinal],
        &observation_fixture,
        &observation_fixture.request_json(),
        &[],
        TEST_NOW_MILLIS_V1,
        transport_parent,
        child_binary,
    );
    let observation_envelope = parse_json(&observation_result)
        .get("authority_envelope")
        .cloned()
        .expect("actual observation authority envelope");
    let replay_fixture = ClientRequestFixtureV1::from_subject_and_manifest(
        TairaAuthorityRoleV1::PublicSoakReplayAdmission,
        public_soak_native_tests::replay_subject(
            &soak_subject,
            completed_at_unix_millis,
            observation_envelope,
        ),
        Value::Array(Vec::new()),
    );
    run_actual_kernel_client_role(
        replay_ordinal,
        &services[replay_ordinal],
        &installations[replay_ordinal],
        &replay_fixture,
        &replay_fixture.request_json(),
        &[],
        TEST_NOW_MILLIS_V1 + 1,
        transport_parent,
        child_binary,
    );
}

#[cfg(target_os = "linux")]
fn exercise_real_kernel_peer_harness(
    services: &[Arc<TairaAuthorityServiceV1>],
    installations: &[TairaAuthorityInstallationV1],
) {
    use std::{process::Stdio, thread, time::Instant};

    if rustix::process::geteuid().as_raw() != 0 {
        eprintln!(
            "SKIPPED privileged eight-role kernel-credential transport coverage: Linux EUID 0 is required; structural isolation coverage still executed"
        );
        return;
    }
    #[cfg(not(all(
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )))]
    {
        eprintln!(
            "SKIPPED privileged eight-role kernel-credential transport coverage: this Linux architecture does not support every native role fixture; structural isolation coverage still executed"
        );
        return;
    }
    let harness_parent = installations[0]
        .state_directory
        .parent()
        .expect("privileged eight-role harness parent");
    assert!(
        installations
            .iter()
            .all(|installation| installation.state_directory.parent() == Some(harness_parent)),
    );
    assert!(
        is_root_owned_nonwritable_executable_directory(harness_parent),
        "privileged role state must be staged below only root-owned, non-writable executable ancestors",
    );
    let transport_parent = tempfile::Builder::new()
        .prefix("kernel-peer-transport-v1-")
        .tempdir_in(harness_parent)
        .expect("secure kernel-peer transport parent");
    fs::set_permissions(transport_parent.path(), fs::Permissions::from_mode(0o711))
        .expect("make kernel-peer transport parent traversable");
    assert!(is_root_owned_nonwritable_executable_directory(
        transport_parent.path()
    ));
    let child_binary = transport_parent.path().join("irohad-kernel-peer-child-v1");
    fs::copy(
        std::env::current_exe().expect("current authority test binary"),
        &child_binary,
    )
    .expect("copy isolated kernel-peer child binary");
    fs::set_permissions(&child_binary, fs::Permissions::from_mode(0o555))
        .expect("make isolated kernel-peer child executable");
    let child_metadata =
        fs::symlink_metadata(&child_binary).expect("inspect isolated kernel-peer child executable");
    assert!(
        child_metadata.is_file()
            && child_metadata.uid() == 0
            && child_metadata.mode() & 0o222 == 0
            && child_metadata.mode() & 0o111 != 0
    );
    eprintln!(
        "EXECUTING privileged eight-role kernel-credential transport coverage with real service, administrator, and client UIDs"
    );

    let run_child = |ordinal: usize, child_uid: u32, expect_rejection: bool| {
        let service = &services[ordinal];
        let installation = &installations[ordinal];
        let socket = transport_parent.path().join(format!("{ordinal:02}.sock"));
        let listener = std::os::unix::net::UnixListener::bind(&socket)
            .expect("bind real kernel-peer listener");
        fs::set_permissions(&socket, fs::Permissions::from_mode(0o666))
            .expect("make real kernel-peer listener connectable");
        listener
            .set_nonblocking(true)
            .expect("set kernel-peer listener nonblocking");

        let mut command = privileged_kernel_peer_child_command(&child_binary, child_uid);
        command
            .arg("--exact")
            .arg(KERNEL_PEER_CHILD_TEST_NAME_V1)
            .arg("--nocapture")
            .current_dir("/")
            .env(KERNEL_PEER_CHILD_SOCKET_ENV_V1, &socket)
            .env(
                KERNEL_PEER_CHILD_BINDING_ENV_V1,
                hex::encode(
                    installation
                        .binding
                        .sha256()
                        .expect("kernel-peer binding digest"),
                ),
            )
            .env(
                KERNEL_PEER_CHILD_NONCE_ENV_V1,
                u8::try_from(ordinal + 1)
                    .expect("kernel-peer nonce")
                    .to_string(),
            )
            .env(
                KERNEL_PEER_CHILD_ROLE_ENV_V1,
                installation.binding.role.as_str(),
            )
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if expect_rejection {
            command.env(KERNEL_PEER_CHILD_REJECTION_ENV_V1, "1");
        }
        let mut child = command.spawn().expect("spawn isolated kernel-peer child");
        let deadline = Instant::now() + Duration::from_secs(5);
        let stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if let Some(status) = child.try_wait().expect("poll kernel-peer child") {
                        panic!("kernel-peer child exited before connect: {status}");
                    }
                    if Instant::now() >= deadline {
                        let _ = child.kill();
                        let _ = child.wait();
                        panic!("kernel-peer child did not connect before timeout");
                    }
                    thread::sleep(Duration::from_millis(5));
                }
                Err(error) => panic!("accept kernel-peer child: {error}"),
            }
        };
        let served = serve_one_kernel_peer_for_test(stream, service);
        let output = child
            .wait_with_output()
            .expect("wait for isolated kernel-peer child");
        drop(listener);
        fs::remove_file(&socket).expect("remove kernel-peer socket");
        assert!(
            output.status.success(),
            "kernel-peer child failed: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        served
    };

    for (ordinal, installation) in installations.iter().enumerate() {
        run_child(ordinal, installation.binding.signer.client_uid, false)
            .expect("role-bound kernel peer must complete canonical qualify");
    }
    let wrong_uid = installations[1].binding.signer.client_uid;
    assert_ne!(wrong_uid, installations[0].binding.signer.client_uid);
    assert_eq!(
        run_child(0, wrong_uid, true),
        Err(TairaAuthorityErrorV1::Binding),
        "server must reject the kernel peer before waiting for a frame"
    );
    exercise_actual_kernel_client_roles(
        services,
        installations,
        transport_parent.path(),
        &child_binary,
    );
    eprintln!(
        "COMPLETED privileged eight-role kernel-credential administrator/authorize/retry/verify coverage"
    );
}

#[cfg(not(target_os = "linux"))]
fn exercise_real_kernel_peer_harness(
    _services: &[Arc<TairaAuthorityServiceV1>],
    _installations: &[TairaAuthorityInstallationV1],
) {
    eprintln!(
        "SKIPPED privileged eight-role kernel-credential transport coverage: Linux is required; structural isolation coverage still executed"
    );
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
fn local_eight_role_structural_harness_with_privileged_transport_when_supported() {
    let parent = eight_role_harness_parent();
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

    exercise_real_kernel_peer_harness(&services, &installations);
}

include!("tests/authorization_assignment_tests.rs");

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

include!("tests/public_soak_recovery_tests.rs");
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
    let post_revoke_historical = service
        .verify_json(
            &verification_json(&fixture, &old_authorization),
            read_only_descriptors(&[&artifact]),
            service
                .public_binding()
                .expect("post-revoke verification binding")
                .signer
                .client_uid,
        )
        .expect("verify preexisting receipt after revoke");
    assert_eq!(post_revoke_historical.status, OperationStatusV1::Ok);
    assert_eq!(result_status(&post_revoke_historical), "valid");
    assert_eq!(
        service.provenance().expect("post-revoke provenance"),
        after_revoke
    );

    let fresh_artifact = create_artifact(parent.path(), "revoked-fresh.json", b"revoked-fresh");
    let fresh_fixture = ClientRequestFixtureV1::new(
        TairaAuthorityRoleV1::RolloutObservation,
        "revoked-fresh",
        &[("revoked-fresh.json", b"revoked-fresh")],
    );
    let fresh_assignment = fresh_fixture.assignment_json(
        &service,
        TEST_NOW_MILLIS_V1,
        TEST_NOW_MILLIS_V1,
        TEST_NOW_MILLIS_V1 + 100,
    );
    assert_eq!(
        service.assign_run_json(&fresh_assignment, TEST_NOW_MILLIS_V1 + 2),
        Err(TairaAuthorityErrorV1::Conflict),
        "a revoked authority must refuse a fresh assignment"
    );
    assert_eq!(
        authorize(
            &service,
            &fresh_fixture.request_json(),
            read_only_descriptors(&[&fresh_artifact]),
            TEST_NOW_MILLIS_V1 + 2,
        ),
        Err(TairaAuthorityErrorV1::Rejected),
        "a revoked authority must refuse a fresh authorization"
    );
    assert_eq!(
        service
            .provenance()
            .expect("fresh refusal leaves revoked provenance unchanged"),
        after_revoke
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
    let recovered_provenance = recovered
        .provenance()
        .expect("recovered historical provenance");
    assert_eq!(recovered_provenance, after_revoke);
    let recovered_historical = recovered
        .verify_json(
            &verification_json(&fixture, &old_authorization),
            read_only_descriptors(&[&artifact]),
            recovered
                .public_binding()
                .expect("recovered verification binding")
                .signer
                .client_uid,
        )
        .expect("verify preexisting receipt after revoked reopen");
    assert_eq!(recovered_historical.status, OperationStatusV1::Ok);
    assert_eq!(result_status(&recovered_historical), "valid");
    assert_eq!(
        recovered
            .provenance()
            .expect("recovered verification provenance"),
        recovered_provenance
    );
    assert_eq!(
        recovered.assign_run_json(&fresh_assignment, TEST_NOW_MILLIS_V1 + 3),
        Err(TairaAuthorityErrorV1::Conflict)
    );
    assert_eq!(
        authorize(
            &recovered,
            &fresh_fixture.request_json(),
            read_only_descriptors(&[&fresh_artifact]),
            TEST_NOW_MILLIS_V1 + 3,
        ),
        Err(TairaAuthorityErrorV1::Rejected)
    );
    assert_eq!(
        recovered
            .provenance()
            .expect("recovered fresh refusals leave provenance unchanged"),
        recovered_provenance
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

include!("tests/deployment_finalization_tests.rs");

include!("tests/privacy_governance_tests.rs");
