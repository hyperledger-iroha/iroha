//! Focused authority protocol, durable-state, and service tests.

use super::{
    TairaAuthorityProvisioningV1, TairaAuthorityRoleV1, TairaAuthorityServiceV1,
    protocol::{
        AuthorityAdminCommandV1, AuthorizeRequestV1, OperationResponseV1, OperationStatusV1,
        ReplayConsumptionV1, TAIRA_AUTHORITY_MAX_FRAME_BYTES_V1, decode_body, decode_frame,
        encode_frame,
    },
    service::{TairaAuthorityErrorV1, digest_parts_sha256},
    store::{load_canonical_records, persist_canonical_once},
};
use super::super::SoftwareSignerWrappingKeyV1;
use norito::json::{Map, Value};
use sha2::{Digest as _, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::Write as _,
    os::{
        fd::OwnedFd,
        unix::fs::{DirBuilderExt as _, OpenOptionsExt as _, PermissionsExt as _},
    },
    path::{Path, PathBuf},
};

const TEST_WRAPPING_KEY_V1: [u8; 32] = [0xA7; 32];
const TEST_NOW_MILLIS_V1: u64 = 1_900_000_000_000;
const RUN_ID_DOMAIN_V1: &[u8] = b"iroha:taira:authority-run-id:v1\0";
const OPERATION_ID_DOMAIN_V1: &[u8] = b"iroha:taira:authority-operation-id:v1\0";

fn wrapping_key() -> SoftwareSignerWrappingKeyV1 {
    SoftwareSignerWrappingKeyV1::try_from_bytes(TEST_WRAPPING_KEY_V1)
        .expect("fixture wrapping key")
}

fn temporary_parent() -> tempfile::TempDir {
    tempfile::tempdir_in(std::env::current_dir().expect("current directory"))
        .expect("secure temporary authority parent")
}

fn provisioning(role: TairaAuthorityRoleV1) -> TairaAuthorityProvisioningV1 {
    let service_uid = rustix::process::geteuid().as_raw();
    TairaAuthorityProvisioningV1 {
        role,
        service_id: format!("taira-authority-{}-test-v1", role.as_str()),
        administrator_id: format!("taira-authority-{}-administrator-test-v1", role.as_str()),
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
    fn new(
        role: TairaAuthorityRoleV1,
        case: &str,
        artifacts: &[(&str, &[u8])],
    ) -> Self {
        let mut subject = Map::new();
        subject.insert("case".into(), Value::from(case));
        subject.insert("schema_version".into(), Value::from(1_u64));
        let subject = Value::Object(subject);
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
        assignment.insert("expires_at_unix_millis".into(), Value::from(expires_at));
        assignment.insert("issued_at_unix_millis".into(), Value::from(issued_at));
        assignment.insert(
            "key_revision".into(),
            Value::from(binding.signer.key_revision),
        );
        assignment.insert(
            "not_before_unix_millis".into(),
            Value::from(not_before),
        );
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
    path
}

fn read_only_descriptors(paths: &[&Path]) -> Vec<OwnedFd> {
    paths
        .iter()
        .map(|path| {
            let file = OpenOptions::new()
                .read(true)
                .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
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
        rustix::process::geteuid().as_raw(),
        now,
    )
}

fn sidecar_bytes(response: &OperationResponseV1, field: &str) -> Vec<u8> {
    let result = parse_json(&response.result_json);
    canonical_json_line(result.get(field).expect("result sidecar"))
}

fn verification_json(fixture: &ClientRequestFixtureV1, authorized: &OperationResponseV1) -> Vec<u8> {
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
