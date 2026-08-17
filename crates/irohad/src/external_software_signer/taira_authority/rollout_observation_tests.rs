use super::*;
use norito::json::{Map, Value};

fn object<const N: usize>(entries: [(&str, Value); N]) -> Value {
    Value::Object(
        entries
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    )
}

fn strings(values: &[&str]) -> Value {
    Value::Array(values.iter().copied().map(Value::from).collect())
}

fn digest(label: &str) -> String {
    hex::encode(Sha256::digest(label.as_bytes()))
}

fn oci(label: &str) -> String {
    format!("sha256:{}", digest(label))
}

fn immutable_plan() -> Value {
    let core = IMMUTABLE_PLAN_V1
        .strip_suffix(b"\n")
        .expect("fixture plan has canonical newline");
    norito::json::from_slice(core).expect("fixture plan parses")
}

fn candidate() -> Value {
    let mut value = Map::new();
    value.insert("archive_sha256".into(), Value::from(digest("archive")));
    value.insert("candidate_oci_digest".into(), Value::from(oci("candidate")));
    value.insert(
        "capability_schema_sha256".into(),
        Value::from(digest("capability-schema")),
    );
    value.insert(
        "cargo_lock_sha256".into(),
        Value::from(FROZEN_CARGO_LOCK_SHA256_V1),
    );
    value.insert(
        "dpn_validator_release_commit".into(),
        Value::from("b".repeat(40)),
    );
    value.insert("irohad_sha256".into(), Value::from(digest("iroha3d")));
    value.insert(
        "protocol_matrix_sha256".into(),
        Value::from(EXACT12_MATRIX_SHA256_V1),
    );
    value.insert("source_commit".into(), Value::from("a".repeat(40)));
    value.insert(
        "workspace_source_manifest_sha256".into(),
        Value::from(digest("source-manifest")),
    );
    let binding = digest_domain_value(CANDIDATE_ID_DOMAIN_V1, &Value::Object(value.clone()))
        .expect("derive candidate binding");
    value.insert(
        "candidate_binding_sha256".into(),
        Value::from(hex::encode(binding)),
    );
    Value::Object(value)
}

fn active(wave_count: usize) -> Vec<&'static str> {
    expected_active_v1(wave_count)
}

fn snapshots(height: u64, active: &[&str], binding: &str, tag: &str) -> Value {
    let block_hash = digest(&format!("{tag}:block"));
    let manifest = digest(&format!("{tag}:manifest"));
    Value::Array(
        ENDPOINTS_V1
            .iter()
            .map(|endpoint| {
                object([
                    ("active_protocols", strings(active)),
                    ("available_protocols", strings(&PROTOCOLS_V1)),
                    ("block_hash", Value::from(block_hash.clone())),
                    ("candidate_binding_sha256", Value::from(binding.to_owned())),
                    ("capability_manifest_sha256", Value::from(manifest.clone())),
                    ("endpoint", Value::from(*endpoint)),
                    ("height", Value::from(height)),
                    ("jindo_assurance", Value::from(JINDO_ASSURANCE_V1)),
                    (
                        "jindo_missing_evidence",
                        strings(&JINDO_MISSING_EVIDENCE_V1),
                    ),
                    ("unavailable_protocols", Value::Array(Vec::new())),
                ])
            })
            .collect(),
    )
}

fn queries(height: u64, tag: &str) -> Value {
    let state = digest(&format!("{tag}:state"));
    Value::Array(
        ENDPOINTS_V1
            .iter()
            .map(|endpoint| {
                object([
                    ("endpoint", Value::from(*endpoint)),
                    ("height", Value::from(height)),
                    ("state_sha256", Value::from(state.clone())),
                ])
            })
            .collect(),
    )
}

fn rejections(height: u64, failure_code: &str) -> Value {
    Value::Array(
        ENDPOINTS_V1
            .iter()
            .map(|endpoint| {
                object([
                    ("endpoint", Value::from(*endpoint)),
                    ("failure_code", Value::from(failure_code)),
                    ("observed_height", Value::from(height)),
                ])
            })
            .collect(),
    )
}

fn canaries(protocols: &[&str], activation: u64, binding: &str, wave_index: usize) -> (Value, u64) {
    let mut maximum = activation;
    let rows = protocols
        .iter()
        .enumerate()
        .map(|(offset, protocol)| {
            let accepted = activation + offset as u64 + 1;
            maximum = maximum.max(accepted);
            let transaction = digest(&format!("wave-{wave_index}:{protocol}:positive"));
            object([
                ("candidate_binding_sha256", Value::from(binding.to_owned())),
                (
                    "negative",
                    object([
                        (
                            "rejections",
                            rejections(accepted, "privacy-malformed-proof"),
                        ),
                        (
                            "transaction_sha256",
                            Value::from(digest(&format!("wave-{wave_index}:{protocol}:negative"))),
                        ),
                    ]),
                ),
                (
                    "positive",
                    object([
                        ("accepted_height", Value::from(accepted)),
                        (
                            "peer_queries",
                            queries(accepted, &format!("wave-{wave_index}:{protocol}:query")),
                        ),
                        (
                            "statement_sha256",
                            Value::from(digest(&format!("wave-{wave_index}:{protocol}:statement"))),
                        ),
                        ("submitted_via", Value::from("public-torii")),
                        ("transaction_sha256", Value::from(transaction.clone())),
                    ]),
                ),
                ("protocol", Value::from(*protocol)),
                (
                    "replay",
                    object([
                        ("rejections", rejections(accepted, "privacy-replay")),
                        ("transaction_sha256", Value::from(transaction)),
                    ]),
                ),
            ])
        })
        .collect();
    (Value::Array(rows), maximum)
}

fn resources(protocols: &[&str]) -> Value {
    Value::Array(
        protocols
            .iter()
            .map(|protocol| {
                let evaluated = if *protocol == "iroha-zk-ams-v1" {
                    ZK_AMS_EVALUATED_KEY_BYTES_V1
                } else {
                    0
                };
                object([
                    ("address_space_bytes", Value::from(512_u64 * 1024 * 1024)),
                    ("elapsed_millis", Value::from(10_000_u64)),
                    ("evaluated_key_publication_bytes", Value::from(evaluated)),
                    ("evaluated_key_retrieval_bytes", Value::from(evaluated)),
                    ("peak_rss_bytes", Value::from(256_u64 * 1024 * 1024)),
                    ("protocol", Value::from(*protocol)),
                    ("transport_bytes", Value::from(1024_u64 * 1024)),
                    ("work_units", Value::from(1_000_000_u64)),
                ])
            })
            .collect(),
    )
}

fn restart(wave_index: usize, stopped: u64, sentinel: u64) -> Value {
    let sentinel_hash = digest(&format!("wave-{wave_index}:sentinel"));
    let successor = sentinel + 1;
    let successor_hash = digest(&format!("wave-{wave_index}:successor"));
    object([
        (
            "peer_finality",
            Value::Array(
                ENDPOINTS_V1
                    .iter()
                    .map(|endpoint| {
                        object([
                            ("block_hash", Value::from(successor_hash.clone())),
                            ("endpoint", Value::from(*endpoint)),
                            ("height", Value::from(successor)),
                        ])
                    })
                    .collect(),
            ),
        ),
        ("recovered_hash", Value::from(sentinel_hash.clone())),
        ("recovered_height", Value::from(sentinel)),
        ("sentinel_hash", Value::from(sentinel_hash)),
        ("sentinel_height", Value::from(sentinel)),
        ("stopped_height", Value::from(stopped)),
        ("successor_hash", Value::from(successor_hash)),
        ("successor_height", Value::from(successor)),
        ("validator", Value::from(VALIDATORS_V1[wave_index - 1])),
    ])
}

#[allow(
    clippy::too_many_lines,
    reason = "the cohesive fixture constructs one complete rollout observation"
)]
fn valid_observation_for_case(case: &str) -> Value {
    let mut candidate = candidate();
    let candidate_object = candidate.as_object_mut().expect("candidate object");
    candidate_object.remove("candidate_binding_sha256");
    candidate_object.insert(
        "archive_sha256".into(),
        Value::from(digest(&format!("archive:{case}"))),
    );
    let candidate_binding = digest_domain_value(
        CANDIDATE_ID_DOMAIN_V1,
        &Value::Object(candidate_object.clone()),
    )
    .expect("derive case candidate binding");
    candidate_object.insert(
        "candidate_binding_sha256".into(),
        Value::from(hex::encode(candidate_binding)),
    );
    let candidate_object = candidate.as_object().expect("candidate object");
    let binding = required_str(candidate_object, "candidate_binding_sha256")
        .expect("candidate binding")
        .to_owned();
    let candidate_oci = required_str(candidate_object, "candidate_oci_digest")
        .expect("candidate OCI")
        .to_owned();
    let plan_sha256 = hex::encode(Sha256::digest(IMMUTABLE_PLAN_V1));
    let baseline_height = 10_u64;
    let mut previous_observation = baseline_height;
    let mut waves = Vec::new();
    for (offset, protocols) in WAVES_V1.iter().enumerate() {
        let wave_index = offset + 1;
        let proposed = previous_observation + 1;
        let activation = proposed + NOTICE_INTERVAL_BLOCKS_V1;
        let observation = activation + OBSERVATION_INTERVAL_BLOCKS_V1;
        let (canaries, maximum_canary_height) =
            canaries(protocols, activation, &binding, wave_index);
        let sentinel = maximum_canary_height + 1;
        let successor = sentinel + 1;
        let active_protocols = active(wave_index);
        waves.push(object([
            ("activate_at_height", Value::from(activation)),
            (
                "activation_transactions",
                Value::Array(
                    protocols
                        .iter()
                        .map(|protocol| {
                            object([
                                ("governance_outcome", Value::from("committed")),
                                ("protocol", Value::from(*protocol)),
                                (
                                    "transaction_sha256",
                                    Value::from(digest(&format!(
                                        "wave-{wave_index}:{protocol}:activation"
                                    ))),
                                ),
                            ])
                        })
                        .collect(),
                ),
            ),
            ("canaries", canaries),
            ("candidate_binding_sha256", Value::from(binding.clone())),
            ("index", Value::from(wave_index)),
            ("label", Value::from(format!("wave-{wave_index}"))),
            ("observation_completed_at_height", Value::from(observation)),
            (
                "observation_snapshots",
                snapshots(
                    observation,
                    &active_protocols,
                    &binding,
                    &format!("wave-{wave_index}:observation"),
                ),
            ),
            (
                "post_activation_snapshots",
                snapshots(
                    activation,
                    &active_protocols,
                    &binding,
                    &format!("wave-{wave_index}:activation"),
                ),
            ),
            (
                "post_restart_snapshots",
                snapshots(
                    successor,
                    &active_protocols,
                    &binding,
                    &format!("wave-{wave_index}:restart"),
                ),
            ),
            (
                "pre_activation_snapshots",
                snapshots(
                    proposed - 1,
                    &active(wave_index - 1),
                    &binding,
                    &format!("wave-{wave_index}:pre"),
                ),
            ),
            ("proposed_at_height", Value::from(proposed)),
            ("protocols", strings(protocols)),
            ("resources", resources(protocols)),
            (
                "restart",
                restart(wave_index, maximum_canary_height, sentinel),
            ),
        ]));
        previous_observation = observation;
    }

    let write_height = previous_observation + 1;
    let privacy_height = write_height + 1;
    let post_snapshots = snapshots(
        privacy_height,
        &active(WAVES_V1.len()),
        &binding,
        "post-cutover",
    );
    let post_manifest = post_snapshots
        .get(0)
        .and_then(|row| row.get("capability_manifest_sha256"))
        .and_then(Value::as_str)
        .expect("post manifest")
        .to_owned();
    let privacy_transaction = digest("post-cutover:privacy");
    let mut root = Map::new();
    root.insert(
        "baseline".into(),
        object([(
            "snapshots",
            snapshots(baseline_height, &[], &binding, "baseline"),
        )]),
    );
    root.insert("candidate".into(), candidate);
    root.insert("completed_at_unix".into(), Value::from(2_000_000_000_u64));
    root.insert("plan_sha256".into(), Value::from(plan_sha256));
    root.insert(
        "post_cutover".into(),
        object([
            (
                "canary",
                object([
                    (
                        "authority_scope",
                        Value::from("dedicated-no-governance-canary"),
                    ),
                    ("bootstrap_authority", Value::from(false)),
                    ("governance_permission_present", Value::from(false)),
                    ("mode", Value::from("signed-write-and-privacy")),
                    (
                        "privacy",
                        object([
                            ("accepted_height", Value::from(privacy_height)),
                            (
                                "peer_queries",
                                queries(privacy_height, "post-cutover:privacy-query"),
                            ),
                            ("protocol", Value::from("verange-transparent-range-v1")),
                            (
                                "statement_sha256",
                                Value::from(digest("post-cutover:privacy-statement")),
                            ),
                            ("submitted_via", Value::from("public-torii")),
                            (
                                "transaction_sha256",
                                Value::from(privacy_transaction.clone()),
                            ),
                        ]),
                    ),
                    (
                        "replay",
                        object([
                            ("rejections", rejections(privacy_height, "privacy-replay")),
                            ("transaction_sha256", Value::from(privacy_transaction)),
                        ]),
                    ),
                    ("skipped", Value::from(false)),
                    (
                        "write",
                        object([
                            ("accepted_height", Value::from(write_height)),
                            (
                                "peer_queries",
                                queries(write_height, "post-cutover:write-query"),
                            ),
                            ("submitted_via", Value::from("public-torii")),
                            (
                                "transaction_sha256",
                                Value::from(digest("post-cutover:write")),
                            ),
                        ]),
                    ),
                ]),
            ),
            (
                "deployed_candidate_oci_digest",
                Value::from(candidate_oci.clone()),
            ),
            (
                "readmitted_candidate_oci_digest",
                Value::from(candidate_oci.clone()),
            ),
            ("snapshots", post_snapshots),
        ]),
    );
    root.insert(
        "rollback".into(),
        object([
            ("armed", Value::from(true)),
            ("invoked", Value::from(false)),
            ("legacy_fallback_used", Value::from(false)),
            (
                "previous_candidate_oci_digest",
                Value::from(oci("previous-candidate")),
            ),
            (
                "previous_capability_manifest_sha256",
                Value::from(digest("previous-capability")),
            ),
            (
                "restore_mode",
                Value::from("immutable-candidate-and-capability-set"),
            ),
        ]),
    );
    root.insert("schema".into(), Value::from(RESULT_SCHEMA_V1));
    root.insert("schema_version".into(), Value::from(1_u64));
    root.insert("started_at_unix".into(), Value::from(1_999_000_000_u64));
    root.insert(
        "terminal".into(),
        object([
            ("final_candidate_oci_digest", Value::from(candidate_oci)),
            (
                "final_capability_manifest_sha256",
                Value::from(post_manifest),
            ),
            ("halt_reason", Value::Null),
            ("halted", Value::from(false)),
            ("publication_authorized", Value::from(true)),
            ("status", Value::from("passed")),
        ]),
    );
    root.insert("waves".into(), Value::Array(waves));
    let rollout_id = digest_domain_value(ROLLOUT_ID_DOMAIN_V1, &Value::Object(root.clone()))
        .expect("derive rollout ID");
    root.insert("rollout_id".into(), Value::from(hex::encode(rollout_id)));
    Value::Object(root)
}

fn valid_observation() -> Value {
    valid_observation_for_case("validator-unit")
}

pub fn valid_subject_for_case(case: &str) -> Value {
    object([
        ("authority_schema", Value::from(AUTHORITY_SCHEMA_V1)),
        ("observation", valid_observation_for_case(case)),
        ("plan", immutable_plan()),
        ("replay_namespace", Value::from(REPLAY_NAMESPACE_V1)),
    ])
}

fn valid_subject() -> Value {
    object([
        ("authority_schema", Value::from(AUTHORITY_SCHEMA_V1)),
        ("observation", valid_observation()),
        ("plan", immutable_plan()),
        ("replay_namespace", Value::from(REPLAY_NAMESPACE_V1)),
    ])
}

#[derive(Clone, Debug)]
enum Segment {
    Key(String),
    Index(usize),
}

fn scalar_paths(value: &Value, path: &mut Vec<Segment>, output: &mut Vec<Vec<Segment>>) {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                path.push(Segment::Key(key.clone()));
                scalar_paths(value, path, output);
                path.pop();
            }
        }
        Value::Array(array) => {
            for (index, value) in array.iter().enumerate() {
                path.push(Segment::Index(index));
                scalar_paths(value, path, output);
                path.pop();
            }
        }
        _ => output.push(path.clone()),
    }
}

fn at_mut<'a>(mut value: &'a mut Value, path: &[Segment]) -> &'a mut Value {
    for segment in path {
        value = match segment {
            Segment::Key(key) => value
                .as_object_mut()
                .and_then(|object| object.get_mut(key))
                .expect("fixture object path"),
            Segment::Index(index) => value
                .as_array_mut()
                .and_then(|array| array.get_mut(*index))
                .expect("fixture array path"),
        };
    }
    value
}

fn mutate_scalar(value: &mut Value) {
    *value = match value {
        Value::Null => Value::from("mutated-null"),
        Value::Bool(value) => Value::from(!*value),
        Value::Number(_) => Value::from(value.as_u64().expect("fixture u64") + 1),
        Value::String(value) => Value::from(format!("x{value}")),
        Value::Array(_) | Value::Object(_) => panic!("only scalar paths are mutated"),
    };
}

fn observation_mut(subject: &mut Value) -> &mut Value {
    subject
        .as_object_mut()
        .and_then(|subject| subject.get_mut("observation"))
        .expect("observation subject")
}

fn redigest_rollout(subject: &mut Value) {
    let observation = observation_mut(subject)
        .as_object_mut()
        .expect("observation object");
    observation.remove("rollout_id").expect("old rollout ID");
    let rollout_id = digest_domain_value(ROLLOUT_ID_DOMAIN_V1, &Value::Object(observation.clone()))
        .expect("re-derive rollout ID");
    observation.insert("rollout_id".into(), Value::from(hex::encode(rollout_id)));
}

fn set_path(subject: &mut Value, path: &[Segment], value: Value) {
    *at_mut(subject, path) = value;
}

fn p(segments: &[&str]) -> Vec<Segment> {
    let mut path = vec![Segment::Key("observation".into())];
    path.extend(segments.iter().map(|value| Segment::Key((*value).into())));
    path
}

#[test]
fn accepts_exact_immutable_four_wave_subject() {
    assert_eq!(
        validate_rollout_observation_subject_v1(&valid_subject()),
        Ok(())
    );
}

#[test]
fn every_individually_mutated_signed_scalar_is_rejected() {
    let subject = valid_subject();
    let mut paths = Vec::new();
    scalar_paths(&subject, &mut Vec::new(), &mut paths);
    assert!(
        paths.len() > 1_000,
        "fixture must exercise the complete subject"
    );
    for path in paths {
        let mut mutated = subject.clone();
        mutate_scalar(at_mut(&mut mutated, &path));
        assert_eq!(
            validate_rollout_observation_subject_v1(&mutated),
            Err(TairaAuthorityErrorV1::Rejected),
            "mutated signed field was accepted: {path:?}"
        );
    }
}

#[test]
fn semantic_substitutions_fail_even_after_the_self_hash_is_rederived() {
    let cases = [
        (
            vec![
                Segment::Key("observation".into()),
                Segment::Key("waves".into()),
                Segment::Index(0),
                Segment::Key("activation_transactions".into()),
                Segment::Index(0),
                Segment::Key("governance_outcome".into()),
            ],
            Value::from("submitted"),
        ),
        (
            vec![
                Segment::Key("observation".into()),
                Segment::Key("waves".into()),
                Segment::Index(0),
                Segment::Key("post_activation_snapshots".into()),
                Segment::Index(0),
                Segment::Key("unavailable_protocols".into()),
            ],
            strings(&["iroha-zk-ams-v1"]),
        ),
        (
            vec![
                Segment::Key("observation".into()),
                Segment::Key("waves".into()),
                Segment::Index(3),
                Segment::Key("resources".into()),
                Segment::Index(1),
                Segment::Key("work_units".into()),
            ],
            Value::from(MAX_WORK_UNITS_V1 + 1),
        ),
        (
            vec![
                Segment::Key("observation".into()),
                Segment::Key("waves".into()),
                Segment::Index(0),
                Segment::Key("restart".into()),
                Segment::Key("recovered_hash".into()),
            ],
            Value::from(digest("stale-recovery")),
        ),
        (
            vec![
                Segment::Key("observation".into()),
                Segment::Key("post_cutover".into()),
                Segment::Key("canary".into()),
                Segment::Key("write".into()),
                Segment::Key("peer_queries".into()),
                Segment::Index(0),
                Segment::Key("state_sha256".into()),
            ],
            Value::from(digest("divergent-write-state")),
        ),
        (p(&["rollback", "armed"]), Value::from(false)),
        (
            p(&["post_cutover", "readmitted_candidate_oci_digest"]),
            Value::from(oci("substituted-candidate")),
        ),
        (
            p(&["candidate", "archive_sha256"]),
            Value::from(digest("substituted-archive")),
        ),
    ];
    for (path, value) in cases {
        let mut subject = valid_subject();
        set_path(&mut subject, &path, value);
        redigest_rollout(&mut subject);
        assert_eq!(
            validate_rollout_observation_subject_v1(&subject),
            Err(TairaAuthorityErrorV1::Rejected),
            "semantic substitution was accepted: {path:?}"
        );
    }
}
